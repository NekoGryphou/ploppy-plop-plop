use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use axum::{
    Extension, Router,
    body::Bytes,
    extract::{ConnectInfo, DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use prost::Message;
use tokio::{net::TcpListener, sync::Mutex};

use crate::{
    HOST_VERSION, PROTOCOL_VERSION,
    auth::{AuthError, AuthenticatedRequest, Authenticator, now_unix, sign_response},
    pairing::{CredentialContext, PairingCode, PairingError},
    power::PowerController,
    protocol::{
        ErrorCode, ErrorResponse, PairRequest, PairResponse, ShutdownRequest, ShutdownResponse,
        StatusRequest, StatusResponse,
    },
    storage::{AcceptedShutdownNonce, CredentialStore, HostIdentity},
};

const CONTENT_TYPE: &str = "application/x-protobuf";

pub struct AppState {
    pub identity: Mutex<HostIdentity>,
    pub pairing: Mutex<PairingCode>,
    pub authenticator: Authenticator,
    pub power: Arc<dyn PowerController>,
    pub store: Arc<dyn CredentialStore>,
    pub hostname: String,
    pub latest_client_version: Mutex<Option<String>>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/v1/status", post(status))
        .route("/v1/pair", post(pair))
        .route("/v1/shutdown", post(shutdown))
        .layer(DefaultBodyLimit::max(64 * 1024))
        .with_state(state)
}

pub async fn serve(listener: TcpListener, state: Arc<AppState>) -> std::io::Result<()> {
    axum::serve(
        listener,
        router(state).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
}

pub async fn bind(port: u16) -> std::io::Result<TcpListener> {
    TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port)).await
}

async fn status(State(state): State<Arc<AppState>>, headers: HeaderMap, body: Bytes) -> Response {
    let authentication = match authenticate(&state, &headers, "POST", "/v1/status", &body).await {
        Ok(authentication) => authentication,
        Err(reason) => return unauthorized(reason),
    };
    if !is_protobuf(&headers) {
        return authenticated_error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ErrorCode::MalformedRequest,
            "Content-Type must be application/x-protobuf.",
            &authentication,
            "/v1/status",
        );
    }
    let request = match StatusRequest::decode(body.clone()) {
        Ok(request) => request,
        Err(_) => {
            return authenticated_error(
                StatusCode::BAD_REQUEST,
                ErrorCode::MalformedRequest,
                "Malformed Protobuf request.",
                &authentication,
                "/v1/status",
            );
        }
    };
    if request.protocol_version != PROTOCOL_VERSION {
        return authenticated_protocol_mismatch(&authentication, "/v1/status");
    }
    record_client_version(&state, &request.client_version).await;
    let identity = state.identity.lock().await;
    authenticated_protobuf(
        StatusCode::OK,
        StatusResponse {
            hostname: state.hostname.clone(),
            host_version: HOST_VERSION.into(),
            protocol_version: PROTOCOL_VERSION,
            paired: identity.credential.is_some(),
            host_id: identity.host_id.to_string(),
        },
        &authentication,
        "/v1/status",
    )
}

async fn pair(
    State(state): State<Arc<AppState>>,
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !is_protobuf(&headers) {
        return error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ErrorCode::MalformedRequest,
            "Content-Type must be application/x-protobuf.",
        );
    }
    let request = match PairRequest::decode(body) {
        Ok(request) => request,
        Err(_) => {
            return error(
                StatusCode::BAD_REQUEST,
                ErrorCode::MalformedRequest,
                "Malformed Protobuf request.",
            );
        }
    };
    if request.protocol_version != PROTOCOL_VERSION {
        return protocol_mismatch();
    }
    if request.session_id.is_empty() {
        {
            let identity = state.identity.lock().await;
            if identity.credential.is_some() && identity.pairing_code.is_none() {
                return error(
                    StatusCode::CONFLICT,
                    ErrorCode::AlreadyPaired,
                    "This host is already paired.",
                );
            }
        }
        let started = match state.pairing.lock().await.start_for_source(
            &request.client_spake2_message,
            connect_info.map(|Extension(ConnectInfo(address))| address.ip()),
        ) {
            Ok(result) => result,
            Err(PairingError::Expired) => {
                return error(
                    StatusCode::GONE,
                    ErrorCode::PairingExpired,
                    "The pairing code expired.",
                );
            }
            Err(PairingError::RateLimited) => {
                return error(
                    StatusCode::TOO_MANY_REQUESTS,
                    ErrorCode::RateLimited,
                    "Too many pairing attempts.",
                );
            }
            Err(_) => {
                return error(
                    StatusCode::UNAUTHORIZED,
                    ErrorCode::Unauthorized,
                    "Pairing failed.",
                );
            }
        };
        let identity = state.identity.lock().await;
        return protobuf(
            StatusCode::OK,
            PairResponse {
                host_spake2_message: started.host_message,
                encryption_nonce: Vec::new(),
                encrypted_credential: Vec::new(),
                hostname: state.hostname.clone(),
                host_version: HOST_VERSION.into(),
                protocol_version: PROTOCOL_VERSION,
                host_id: identity.host_id.to_string(),
                session_id: started.session_id.to_vec(),
            },
        );
    }
    let host_id = state.identity.lock().await.host_id.to_string();
    let mut pairing = state.pairing.lock().await;
    let was_completed = pairing.is_completed(&request.session_id);
    let result = match pairing.finish(
        &request.session_id,
        &request.client_confirmation,
        &CredentialContext {
            hostname: &state.hostname,
            host_version: HOST_VERSION,
            protocol_version: PROTOCOL_VERSION,
            host_id: &host_id,
        },
    ) {
        Ok(result) => result,
        Err(PairingError::Expired) => {
            return error(
                StatusCode::GONE,
                ErrorCode::PairingExpired,
                "The pairing code expired.",
            );
        }
        Err(PairingError::RateLimited) => {
            return error(
                StatusCode::TOO_MANY_REQUESTS,
                ErrorCode::RateLimited,
                "Too many pairing attempts.",
            );
        }
        Err(_) => {
            return error(
                StatusCode::UNAUTHORIZED,
                ErrorCode::Unauthorized,
                "Pairing failed.",
            );
        }
    };
    if was_completed {
        let identity = state.identity.lock().await;
        return pairing_response(&state, &identity, &request.session_id, result);
    }
    let mut identity = state.identity.lock().await;
    let previous_identity = identity.clone();
    identity.credential = Some(result.credential);
    identity.pairing_code = None;
    identity.pairing_created_at = 0;
    identity.accepted_shutdown_nonces.clear();
    if let Err(error_value) = state.store.save(&identity) {
        tracing::error!(error = %error_value, "failed to persist paired credential");
        *identity = previous_identity;
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::Unspecified,
            "Could not save pairing state.",
        );
    }
    pairing
        .commit(
            &request.session_id,
            &request.client_confirmation,
            result.clone(),
        )
        .expect("a prepared pairing session remains present while its lock is held");
    tracing::info!(host_id = %identity.host_id, "pairing succeeded");
    pairing_response(&state, &identity, &request.session_id, result)
}

fn pairing_response(
    state: &AppState,
    identity: &HostIdentity,
    session_id: &[u8],
    result: crate::pairing::PairingResult,
) -> Response {
    protobuf(
        StatusCode::OK,
        PairResponse {
            host_spake2_message: result.host_message,
            encryption_nonce: result.nonce.to_vec(),
            encrypted_credential: result.encrypted_credential,
            hostname: state.hostname.clone(),
            host_version: HOST_VERSION.into(),
            protocol_version: PROTOCOL_VERSION,
            host_id: identity.host_id.to_string(),
            session_id: session_id.to_vec(),
        },
    )
}

async fn shutdown(State(state): State<Arc<AppState>>, headers: HeaderMap, body: Bytes) -> Response {
    let authentication = match authenticate(&state, &headers, "POST", "/v1/shutdown", &body).await {
        Ok(authentication) => authentication,
        Err(reason) => return unauthorized(reason),
    };
    if !is_protobuf(&headers) {
        return authenticated_error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ErrorCode::MalformedRequest,
            "Content-Type must be application/x-protobuf.",
            &authentication,
            "/v1/shutdown",
        );
    }
    let request = match ShutdownRequest::decode(body.clone()) {
        Ok(request) => request,
        Err(_) => {
            return authenticated_error(
                StatusCode::BAD_REQUEST,
                ErrorCode::MalformedRequest,
                "Malformed Protobuf request.",
                &authentication,
                "/v1/shutdown",
            );
        }
    };
    if request.protocol_version != PROTOCOL_VERSION {
        return authenticated_protocol_mismatch(&authentication, "/v1/shutdown");
    }
    record_client_version(&state, &request.client_version).await;
    match state.power.shutdown().await {
        Ok(()) => authenticated_protobuf(
            StatusCode::ACCEPTED,
            ShutdownResponse { accepted: true },
            &authentication,
            "/v1/shutdown",
        ),
        Err(error_value) => {
            tracing::error!(error = %error_value, "local shutdown API rejected request");
            authenticated_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ErrorCode::ShutdownFailed,
                "Windows rejected the shutdown request.",
                &authentication,
                "/v1/shutdown",
            )
        }
    }
}

async fn record_client_version(state: &AppState, version: &str) {
    if crate::versioning::compare(version, HOST_VERSION)
        == crate::versioning::VersionRelation::Unknown
    {
        return;
    }
    if state.latest_client_version.lock().await.as_deref() == Some(version) {
        return;
    }
    let mut identity = state.identity.lock().await;
    if identity.last_client_version.as_deref() != Some(version) {
        let mut updated = identity.clone();
        updated.last_client_version = Some(version.to_owned());
        if let Err(error) = state.store.save(&updated) {
            tracing::warn!(%error, "could not persist the authenticated plugin version");
        } else {
            *identity = updated;
        }
    }
    *state.latest_client_version.lock().await = Some(version.to_owned());
}

async fn authenticate(
    state: &AppState,
    headers: &HeaderMap,
    method: &str,
    path: &str,
    body: &[u8],
) -> Result<AuthenticationContext, AuthError> {
    let timestamp = header(headers, "x-decky-timestamp")
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or(AuthError::Malformed)?;
    let nonce = header(headers, "x-decky-nonce")
        .and_then(|value| hex::decode(value).ok())
        .ok_or(AuthError::Malformed)?;
    let signature = header(headers, "x-decky-signature")
        .and_then(|value| hex::decode(value).ok())
        .ok_or(AuthError::Malformed)?;
    let credential = state
        .identity
        .lock()
        .await
        .credential
        .ok_or(AuthError::Invalid)?;
    state
        .authenticator
        .verify(
            &credential,
            AuthenticatedRequest {
                timestamp,
                nonce: &nonce,
                signature: &signature,
                method,
                path,
                body,
            },
            now_unix(),
        )
        .await?;
    let nonce: [u8; crate::auth::NONCE_LENGTH] = nonce
        .as_slice()
        .try_into()
        .map_err(|_| AuthError::Malformed)?;
    if path == "/v1/shutdown" {
        let mut identity = state.identity.lock().await;
        identity.accepted_shutdown_nonces.retain(|seen| {
            now_unix().saturating_sub(seen.timestamp) <= crate::auth::CLOCK_WINDOW.as_secs()
        });
        if identity
            .accepted_shutdown_nonces
            .iter()
            .any(|seen| seen.nonce.as_slice() == nonce)
        {
            return Err(AuthError::Replay);
        }
        identity
            .accepted_shutdown_nonces
            .push(AcceptedShutdownNonce { timestamp, nonce });
        if state.store.save(&identity).is_err() {
            identity.accepted_shutdown_nonces.pop();
            return Err(AuthError::Persistence);
        }
    }
    Ok(AuthenticationContext { credential, nonce })
}

struct AuthenticationContext {
    credential: [u8; 32],
    nonce: [u8; crate::auth::NONCE_LENGTH],
}

fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}
fn is_protobuf(headers: &HeaderMap) -> bool {
    header(headers, "content-type").is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|mime| mime.trim().eq_ignore_ascii_case(CONTENT_TYPE))
    })
}
fn unauthorized(reason: AuthError) -> Response {
    tracing::warn!(reason = %reason, "request authentication rejected");
    if reason == AuthError::Persistence {
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::Unspecified,
            "Authentication state could not be saved.",
        );
    }
    if reason == AuthError::RateLimited {
        return error(
            StatusCode::TOO_MANY_REQUESTS,
            ErrorCode::RateLimited,
            "Too many authentication failures.",
        );
    }
    let status = if reason == AuthError::Replay {
        StatusCode::CONFLICT
    } else {
        StatusCode::UNAUTHORIZED
    };
    error(
        status,
        ErrorCode::Unauthorized,
        "Request authentication failed.",
    )
}
fn protocol_mismatch() -> Response {
    error(
        StatusCode::UPGRADE_REQUIRED,
        ErrorCode::ProtocolMismatch,
        "DeckyMyRigHost and the Decky plugin use incompatible protocol versions.",
    )
}
fn authenticated_protocol_mismatch(authentication: &AuthenticationContext, path: &str) -> Response {
    authenticated_error(
        StatusCode::UPGRADE_REQUIRED,
        ErrorCode::ProtocolMismatch,
        "DeckyMyRigHost and the Decky plugin use incompatible protocol versions.",
        authentication,
        path,
    )
}
fn authenticated_error(
    status: StatusCode,
    code: ErrorCode,
    message: &str,
    authentication: &AuthenticationContext,
    path: &str,
) -> Response {
    authenticated_protobuf(
        status,
        ErrorResponse {
            code: code as i32,
            message: message.into(),
        },
        authentication,
        path,
    )
}
fn error(status: StatusCode, code: ErrorCode, message: &str) -> Response {
    protobuf(
        status,
        ErrorResponse {
            code: code as i32,
            message: message.into(),
        },
    )
}
fn protobuf<M: Message>(status: StatusCode, message: M) -> Response {
    let mut response = (status, message.encode_to_vec()).into_response();
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        CONTENT_TYPE.parse().unwrap(),
    );
    response
}

fn authenticated_protobuf<M: Message>(
    status: StatusCode,
    message: M,
    authentication: &AuthenticationContext,
    path: &str,
) -> Response {
    let body = message.encode_to_vec();
    let signature = sign_response(
        &authentication.credential,
        &authentication.nonce,
        path,
        status.as_u16(),
        &body,
    )
    .expect("fixed response authentication fields are valid");
    let mut response = (status, body).into_response();
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        CONTENT_TYPE
            .parse()
            .expect("constant content type is valid"),
    );
    response.headers_mut().insert(
        "x-decky-response-signature",
        hex::encode(signature)
            .parse()
            .expect("hex response signature is a valid header"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        auth::{random_nonce, sign, verify_response},
        power::MockPowerController,
        storage::DevelopmentStore,
    };
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use chacha20poly1305::{
        ChaCha20Poly1305, KeyInit, Nonce,
        aead::{Aead, Payload},
    };
    use hkdf::Hkdf;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    use spake2::{Ed25519Group, Identity, Password, Spake2};
    use tempfile::tempdir;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tower::ServiceExt;
    use uuid::Uuid;

    struct FailOnceStore {
        inner: DevelopmentStore,
        fail_next: std::sync::atomic::AtomicBool,
    }

    impl CredentialStore for FailOnceStore {
        fn load_or_create(&self) -> std::io::Result<HostIdentity> {
            self.inner.load_or_create()
        }

        fn save(&self, identity: &HostIdentity) -> std::io::Result<()> {
            if self
                .fail_next
                .swap(false, std::sync::atomic::Ordering::SeqCst)
            {
                Err(std::io::Error::other("injected persistence failure"))
            } else {
                self.inner.save(identity)
            }
        }
    }

    fn state(credential: Option<[u8; 32]>) -> Arc<AppState> {
        let path = tempdir().unwrap().keep().join("identity.json");
        Arc::new(AppState {
            identity: Mutex::new(HostIdentity {
                host_id: Uuid::nil(),
                credential,
                pairing_code: Some("123456".into()),
                pairing_created_at: now_unix(),
                accepted_shutdown_nonces: Vec::new(),
                last_client_version: None,
            }),
            pairing: Mutex::new(PairingCode::generate()),
            authenticator: Authenticator::default(),
            power: Arc::new(MockPowerController::default()),
            store: Arc::new(DevelopmentStore { path: path.clone() }),
            hostname: "test-pc".into(),
            latest_client_version: Mutex::new(None),
        })
    }

    fn authenticated_request(path: &str, body: Vec<u8>, secret: &[u8]) -> Request<Body> {
        let timestamp = now_unix();
        let nonce = random_nonce();
        let signature = sign(secret, timestamp, &nonce, "POST", path, &body).unwrap();
        Request::post(path)
            .header("content-type", CONTENT_TYPE)
            .header("x-decky-timestamp", timestamp)
            .header("x-decky-nonce", hex::encode(nonce))
            .header("x-decky-signature", hex::encode(signature))
            .body(Body::from(body))
            .unwrap()
    }

    #[tokio::test]
    async fn authenticated_status_succeeds() {
        let secret = [8; 32];
        let state = state(Some(secret));
        let app = router(state.clone());
        let path = "/v1/status";
        let body = StatusRequest {
            protocol_version: PROTOCOL_VERSION,
            client_version: HOST_VERSION.into(),
        }
        .encode_to_vec();
        let timestamp = now_unix();
        let nonce = [6; 16];
        let signature = sign(&secret, timestamp, &nonce, "POST", path, &body).unwrap();
        let request = Request::post(path)
            .header("content-type", CONTENT_TYPE)
            .header("x-decky-timestamp", timestamp)
            .header("x-decky-nonce", hex::encode(nonce))
            .header("x-decky-signature", hex::encode(signature))
            .body(Body::from(body))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response_signature = hex::decode(
            response.headers()["x-decky-response-signature"]
                .to_str()
                .unwrap(),
        )
        .unwrap();
        let response_body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        verify_response(
            &secret,
            &nonce,
            path,
            StatusCode::OK.as_u16(),
            &response_body,
            &response_signature,
        )
        .unwrap();
        assert_eq!(
            state.latest_client_version.lock().await.as_deref(),
            Some(HOST_VERSION)
        );
        assert_eq!(
            state.identity.lock().await.last_client_version.as_deref(),
            Some(HOST_VERSION)
        );
    }

    #[tokio::test]
    async fn pairing_completion_can_retry_after_persistence_recovers() {
        let path = tempdir().unwrap().keep().join("identity.json");
        let store = Arc::new(FailOnceStore {
            inner: DevelopmentStore { path: path.clone() },
            fail_next: true.into(),
        });
        let state = Arc::new(AppState {
            identity: Mutex::new(HostIdentity {
                host_id: Uuid::nil(),
                credential: None,
                pairing_code: Some("123456".into()),
                pairing_created_at: now_unix(),
                accepted_shutdown_nonces: Vec::new(),
                last_client_version: None,
            }),
            pairing: Mutex::new(PairingCode::from_code("123456".into()).unwrap()),
            authenticator: Authenticator::default(),
            power: Arc::new(MockPowerController::default()),
            store: store.clone(),
            hostname: "persistence-test".into(),
            latest_client_version: Mutex::new(None),
        });
        let app = router(state);
        let (client, client_message) = Spake2::<Ed25519Group>::start_a(
            &Password::new(b"123456"),
            &Identity::new(b"decky-client"),
            &Identity::new(b"decky-host"),
        );
        let start_body = PairRequest {
            protocol_version: PROTOCOL_VERSION,
            client_version: HOST_VERSION.into(),
            client_spake2_message: client_message.clone(),
            session_id: Vec::new(),
            client_confirmation: Vec::new(),
        }
        .encode_to_vec();
        let start_request = Request::post("/v1/pair")
            .header("content-type", CONTENT_TYPE)
            .body(Body::from(start_body))
            .unwrap();
        let start_response = app.clone().oneshot(start_request).await.unwrap();
        let start_bytes = to_bytes(start_response.into_body(), 64 * 1024)
            .await
            .unwrap();
        let started = PairResponse::decode(start_bytes).unwrap();
        let shared = client.finish(&started.host_spake2_message).unwrap();
        let mut confirmation = <Hmac<Sha256> as Mac>::new_from_slice(&shared).unwrap();
        confirmation.update(b"deckymyrig-pairing-confirm-v1\0");
        confirmation.update(&client_message);
        confirmation.update(&started.host_spake2_message);
        confirmation.update(&started.session_id);
        let finish_body = PairRequest {
            protocol_version: PROTOCOL_VERSION,
            client_version: HOST_VERSION.into(),
            client_spake2_message: Vec::new(),
            session_id: started.session_id,
            client_confirmation: confirmation.finalize().into_bytes().to_vec(),
        }
        .encode_to_vec();
        let finish_request = || {
            Request::post("/v1/pair")
                .header("content-type", CONTENT_TYPE)
                .body(Body::from(finish_body.clone()))
                .unwrap()
        };

        assert_eq!(
            app.clone()
                .oneshot(finish_request())
                .await
                .unwrap()
                .status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            app.oneshot(finish_request()).await.unwrap().status(),
            StatusCode::OK
        );
        assert!(store.load_or_create().unwrap().credential.is_some());
    }

    #[tokio::test]
    async fn authenticated_protocol_errors_are_signed() {
        let secret = [8; 32];
        let path = "/v1/status";
        let body = StatusRequest {
            protocol_version: 99,
            client_version: HOST_VERSION.into(),
        }
        .encode_to_vec();
        let timestamp = now_unix();
        let nonce = [11; 16];
        let signature = sign(&secret, timestamp, &nonce, "POST", path, &body).unwrap();
        let request = Request::post(path)
            .header("content-type", CONTENT_TYPE)
            .header("x-decky-timestamp", timestamp)
            .header("x-decky-nonce", hex::encode(nonce))
            .header("x-decky-signature", hex::encode(signature))
            .body(Body::from(body))
            .unwrap();
        let response = router(state(Some(secret))).oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UPGRADE_REQUIRED);
        let response_signature = hex::decode(
            response.headers()["x-decky-response-signature"]
                .to_str()
                .unwrap(),
        )
        .unwrap();
        let response_body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        verify_response(
            &secret,
            &nonce,
            path,
            StatusCode::UPGRADE_REQUIRED.as_u16(),
            &response_body,
            &response_signature,
        )
        .unwrap();
        assert_eq!(
            ErrorResponse::decode(response_body).unwrap().code,
            ErrorCode::ProtocolMismatch as i32
        );
    }

    #[tokio::test]
    async fn authenticated_shutdown_replay_is_rejected_after_host_restart() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("identity.json");
        let store = DevelopmentStore { path: path.clone() };
        let secret = [8; 32];
        store
            .save(&HostIdentity {
                host_id: Uuid::nil(),
                credential: Some(secret),
                pairing_code: None,
                pairing_created_at: 0,
                accepted_shutdown_nonces: Vec::new(),
                last_client_version: None,
            })
            .unwrap();
        let body = ShutdownRequest {
            protocol_version: PROTOCOL_VERSION,
            client_version: HOST_VERSION.into(),
        }
        .encode_to_vec();
        let timestamp = now_unix();
        let nonce = [7; 16];
        let signature = sign(&secret, timestamp, &nonce, "POST", "/v1/shutdown", &body).unwrap();
        let request = || {
            Request::post("/v1/shutdown")
                .header("content-type", CONTENT_TYPE)
                .header("x-decky-timestamp", timestamp)
                .header("x-decky-nonce", hex::encode(nonce))
                .header("x-decky-signature", hex::encode(signature))
                .body(Body::from(body.clone()))
                .unwrap()
        };
        let make_state = || {
            let store: Arc<dyn CredentialStore> = Arc::new(DevelopmentStore { path: path.clone() });
            Arc::new(AppState {
                identity: Mutex::new(store.load_or_create().unwrap()),
                pairing: Mutex::new(PairingCode::generate()),
                authenticator: Authenticator::default(),
                power: Arc::new(MockPowerController::default()),
                store,
                hostname: "restart-test".into(),
                latest_client_version: Mutex::new(None),
            })
        };

        assert_eq!(
            router(make_state())
                .oneshot(request())
                .await
                .unwrap()
                .status(),
            StatusCode::ACCEPTED
        );
        assert_eq!(
            router(make_state())
                .oneshot(request())
                .await
                .unwrap()
                .status(),
            StatusCode::CONFLICT
        );
    }

    #[tokio::test]
    async fn shutdown_requires_authentication() {
        let app = router(state(Some([8; 32])));
        let request = Request::post("/v1/shutdown")
            .header("content-type", CONTENT_TYPE)
            .body(Body::from(
                ShutdownRequest {
                    protocol_version: PROTOCOL_VERSION,
                    client_version: HOST_VERSION.into(),
                }
                .encode_to_vec(),
            ))
            .unwrap();
        assert_eq!(
            app.oneshot(request).await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn malformed_protobuf_and_protocol_mismatch_are_structured() {
        let secret = [8; 32];
        let request = authenticated_request("/v1/status", vec![0xff], &secret);
        assert_eq!(
            router(state(Some(secret)))
                .oneshot(request)
                .await
                .unwrap()
                .status(),
            StatusCode::BAD_REQUEST
        );
        let body = StatusRequest {
            protocol_version: 99,
            client_version: HOST_VERSION.into(),
        }
        .encode_to_vec();
        let request = authenticated_request("/v1/status", body, &secret);
        assert_eq!(
            router(state(Some(secret)))
                .oneshot(request)
                .await
                .unwrap()
                .status(),
            StatusCode::UPGRADE_REQUIRED
        );
    }

    #[tokio::test]
    async fn binding_same_port_twice_fails() {
        let first = match bind(0).await {
            Ok(listener) => listener,
            Err(error_value) if error_value.kind() == std::io::ErrorKind::PermissionDenied => {
                return;
            }
            Err(error_value) => panic!("unexpected bind failure: {error_value}"),
        };
        let port = first.local_addr().unwrap().port();
        assert!(bind(port).await.is_err());
    }

    async fn tcp_post(
        address: SocketAddr,
        path: &str,
        body: &[u8],
        authentication_headers: &[(&str, String)],
    ) -> (u16, Vec<u8>) {
        let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
        let mut request = format!(
            "POST {path} HTTP/1.1\r\nHost: {address}\r\nContent-Type: {CONTENT_TYPE}\r\nContent-Length: {}\r\nConnection: close\r\n",
            body.len()
        );
        for (name, value) in authentication_headers {
            request.push_str(&format!("{name}: {value}\r\n"));
        }
        request.push_str("\r\n");
        stream.write_all(request.as_bytes()).await.unwrap();
        stream.write_all(body).await.unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await.unwrap();
        let header_end = response
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .unwrap();
        let headers = std::str::from_utf8(&response[..header_end]).unwrap();
        let status = headers
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        (status, response[header_end + 4..].to_vec())
    }

    fn tcp_auth_headers(path: &str, body: &[u8], credential: &[u8]) -> Vec<(&'static str, String)> {
        let timestamp = now_unix();
        let nonce = random_nonce();
        let signature = sign(credential, timestamp, &nonce, "POST", path, body).unwrap();
        vec![
            ("x-decky-timestamp", timestamp.to_string()),
            ("x-decky-nonce", hex::encode(nonce)),
            ("x-decky-signature", hex::encode(signature)),
        ]
    }

    #[tokio::test]
    async fn real_tcp_pair_status_and_mock_shutdown_flow() {
        const CONFIRMATION_DOMAIN: &[u8] = b"deckymyrig-pairing-confirm-v1\0";
        const CREDENTIAL_INFO: &[u8] = b"deckymyrig-pairing-credential-v1";
        let path = tempdir().unwrap().keep().join("identity.json");
        let power = Arc::new(MockPowerController::default());
        let state = Arc::new(AppState {
            identity: Mutex::new(HostIdentity {
                host_id: Uuid::nil(),
                credential: None,
                pairing_code: None,
                pairing_created_at: now_unix(),
                accepted_shutdown_nonces: Vec::new(),
                last_client_version: None,
            }),
            pairing: Mutex::new(PairingCode::generate()),
            authenticator: Authenticator::default(),
            power: power.clone(),
            store: Arc::new(DevelopmentStore { path: path.clone() }),
            hostname: "tcp-test-pc".into(),
            latest_client_version: Mutex::new(None),
        });
        let managed = crate::management::generate_pairing_code(&state)
            .await
            .unwrap();
        let code = managed.code.unwrap();
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(serve(listener, state));

        let (status_code, response) = tcp_post(address, "/v1/pairing-code", &[], &[]).await;
        assert_eq!(status_code, 404);
        assert!(
            !response
                .windows(code.len())
                .any(|window| window == code.as_bytes())
        );

        let (client, client_message) = Spake2::<Ed25519Group>::start_a(
            &Password::new(code.as_bytes()),
            &Identity::new(b"decky-client"),
            &Identity::new(b"decky-host"),
        );
        let initial = PairRequest {
            protocol_version: PROTOCOL_VERSION,
            client_version: HOST_VERSION.into(),
            client_spake2_message: client_message.clone(),
            session_id: Vec::new(),
            client_confirmation: Vec::new(),
        }
        .encode_to_vec();
        let (status_code, response) = tcp_post(address, "/v1/pair", &initial, &[]).await;
        assert_eq!(status_code, 200);
        let started = PairResponse::decode(response.as_slice()).unwrap();
        let shared = client.finish(&started.host_spake2_message).unwrap();
        let mut confirmation = <Hmac<Sha256> as Mac>::new_from_slice(&shared).unwrap();
        confirmation.update(CONFIRMATION_DOMAIN);
        confirmation.update(&client_message);
        confirmation.update(&started.host_spake2_message);
        confirmation.update(&started.session_id);
        let finish = PairRequest {
            protocol_version: PROTOCOL_VERSION,
            client_version: HOST_VERSION.into(),
            client_spake2_message: Vec::new(),
            session_id: started.session_id,
            client_confirmation: confirmation.finalize().into_bytes().to_vec(),
        }
        .encode_to_vec();
        let (first_completion, second_completion) = tokio::join!(
            tcp_post(address, "/v1/pair", &finish, &[]),
            tcp_post(address, "/v1/pair", &finish, &[]),
        );
        let (status_code, response) = first_completion;
        assert_eq!(status_code, 200);
        assert_eq!(second_completion.0, 200);
        assert_eq!(second_completion.1, response);
        let mut tampered_finish = PairRequest::decode(finish.as_slice()).unwrap();
        tampered_finish.client_confirmation[0] ^= 1;
        assert_eq!(
            tcp_post(address, "/v1/pair", &tampered_finish.encode_to_vec(), &[],)
                .await
                .0,
            401
        );
        let paired = PairResponse::decode(response.as_slice()).unwrap();
        let mut key = [0_u8; 32];
        Hkdf::<Sha256>::new(None, &shared)
            .expand(CREDENTIAL_INFO, &mut key)
            .unwrap();
        let credential = ChaCha20Poly1305::new((&key).into())
            .decrypt(
                Nonce::from_slice(&paired.encryption_nonce),
                Payload {
                    msg: &paired.encrypted_credential,
                    aad: &crate::pairing::credential_aad(
                        &paired.host_spake2_message,
                        &paired.session_id,
                        &CredentialContext {
                            hostname: &paired.hostname,
                            host_version: &paired.host_version,
                            protocol_version: paired.protocol_version,
                            host_id: &paired.host_id,
                        },
                    )
                    .unwrap(),
                },
            )
            .unwrap();

        let status_body = StatusRequest {
            protocol_version: PROTOCOL_VERSION,
            client_version: HOST_VERSION.into(),
        }
        .encode_to_vec();
        let headers = tcp_auth_headers("/v1/status", &status_body, &credential);
        let (status_code, response) = tcp_post(address, "/v1/status", &status_body, &headers).await;
        assert_eq!(status_code, 200);
        assert_eq!(
            StatusResponse::decode(response.as_slice())
                .unwrap()
                .hostname,
            "tcp-test-pc"
        );

        let shutdown_body = ShutdownRequest {
            protocol_version: PROTOCOL_VERSION,
            client_version: HOST_VERSION.into(),
        }
        .encode_to_vec();
        let headers = tcp_auth_headers("/v1/shutdown", &shutdown_body, &credential);
        let (status_code, _) = tcp_post(address, "/v1/shutdown", &shutdown_body, &headers).await;
        assert_eq!(status_code, 202);
        assert!(power.was_requested());

        server.abort();
        let _ = server.await;

        let restarted_store: Arc<dyn CredentialStore> =
            Arc::new(DevelopmentStore { path: path.clone() });
        let restarted_identity = restarted_store.load_or_create().unwrap();
        assert_eq!(
            restarted_identity.credential.as_ref().unwrap().as_slice(),
            credential.as_slice()
        );
        let restarted = Arc::new(AppState {
            identity: Mutex::new(restarted_identity),
            pairing: Mutex::new(PairingCode::generate()),
            authenticator: Authenticator::default(),
            power: Arc::new(MockPowerController::default()),
            store: restarted_store,
            hostname: "tcp-test-pc".into(),
            latest_client_version: Mutex::new(None),
        });
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let restarted_address = listener.local_addr().unwrap();
        let restarted_server = tokio::spawn(serve(listener, restarted));

        let headers = tcp_auth_headers("/v1/status", &status_body, &credential);
        let (status_code, _) =
            tcp_post(restarted_address, "/v1/status", &status_body, &headers).await;
        assert_eq!(status_code, 200);
        let invalid_headers = tcp_auth_headers("/v1/status", &status_body, &[99_u8; 32]);
        let (status_code, _) = tcp_post(
            restarted_address,
            "/v1/status",
            &status_body,
            &invalid_headers,
        )
        .await;
        assert_eq!(status_code, 401);
        let (status_code, _) = tcp_post(restarted_address, "/v1/pair", &initial, &[]).await;
        assert_eq!(status_code, 409);

        restarted_server.abort();
    }

    #[tokio::test]
    async fn real_tcp_pairing_enforces_expiration_and_attempt_limit() {
        let make_state = |pairing: PairingCode| {
            Arc::new(AppState {
                identity: Mutex::new(HostIdentity {
                    host_id: Uuid::new_v4(),
                    credential: None,
                    pairing_code: Some("123456".into()),
                    pairing_created_at: now_unix(),
                    accepted_shutdown_nonces: Vec::new(),
                    last_client_version: None,
                }),
                pairing: Mutex::new(pairing),
                authenticator: Authenticator::default(),
                power: Arc::new(MockPowerController::default()),
                store: Arc::new(DevelopmentStore {
                    path: tempdir().unwrap().keep().join("identity.json"),
                }),
                hostname: "pairing-boundary-test".into(),
                latest_client_version: Mutex::new(None),
            })
        };
        let (_, client_message) = Spake2::<Ed25519Group>::start_a(
            &Password::new(b"123456"),
            &Identity::new(b"decky-client"),
            &Identity::new(b"decky-host"),
        );
        let request = PairRequest {
            protocol_version: PROTOCOL_VERSION,
            client_version: HOST_VERSION.into(),
            client_spake2_message: client_message,
            session_id: Vec::new(),
            client_confirmation: Vec::new(),
        }
        .encode_to_vec();

        let expired =
            PairingCode::from_code_with_age("123456".into(), std::time::Duration::from_secs(301))
                .unwrap();
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(serve(listener, make_state(expired)));
        assert_eq!(tcp_post(address, "/v1/pair", &request, &[]).await.0, 410);
        task.abort();

        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(serve(
            listener,
            make_state(PairingCode::from_code("123456".into()).unwrap()),
        ));
        for _ in 0..8 {
            assert_eq!(tcp_post(address, "/v1/pair", &request, &[]).await.0, 200);
        }
        assert_eq!(tcp_post(address, "/v1/pair", &request, &[]).await.0, 429);
        task.abort();
    }
}
