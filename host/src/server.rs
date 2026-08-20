use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use prost::Message;
use tokio::{net::TcpListener, sync::Mutex};

use crate::{
    HOST_VERSION, PROTOCOL_VERSION,
    auth::{AuthError, AuthenticatedRequest, Authenticator, now_unix},
    pairing::{PairingCode, PairingError},
    power::PowerController,
    protocol::{
        ErrorCode, ErrorResponse, PairRequest, PairResponse, ShutdownRequest, ShutdownResponse,
        StatusRequest, StatusResponse,
    },
    storage::{CredentialStore, HostIdentity},
};

const CONTENT_TYPE: &str = "application/x-protobuf";

pub struct AppState {
    pub identity: Mutex<HostIdentity>,
    pub pairing: Mutex<PairingCode>,
    pub authenticator: Authenticator,
    pub power: Arc<dyn PowerController>,
    pub store: Arc<dyn CredentialStore>,
    pub hostname: String,
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
    axum::serve(listener, router(state)).await
}

pub async fn bind(port: u16) -> std::io::Result<TcpListener> {
    TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port)).await
}

async fn status(State(state): State<Arc<AppState>>, headers: HeaderMap, body: Bytes) -> Response {
    if !is_protobuf(&headers) {
        return error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ErrorCode::MalformedRequest,
            "Content-Type must be application/x-protobuf.",
        );
    }
    let request = match StatusRequest::decode(body.clone()) {
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
    if let Err(reason) = authenticate(&state, &headers, "POST", "/v1/status", &body).await {
        return unauthorized(reason);
    }
    let identity = state.identity.lock().await;
    protobuf(
        StatusCode::OK,
        StatusResponse {
            hostname: state.hostname.clone(),
            host_version: HOST_VERSION.into(),
            protocol_version: PROTOCOL_VERSION,
            paired: identity.credential.is_some(),
            host_id: identity.host_id.to_string(),
        },
    )
}

async fn pair(State(state): State<Arc<AppState>>, headers: HeaderMap, body: Bytes) -> Response {
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
    if state.identity.lock().await.credential.is_some() {
        return error(
            StatusCode::CONFLICT,
            ErrorCode::AlreadyPaired,
            "This host is already paired.",
        );
    }
    if request.session_id.is_empty() {
        let started = match state
            .pairing
            .lock()
            .await
            .start(&request.client_spake2_message)
        {
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
    let result = match state
        .pairing
        .lock()
        .await
        .finish(&request.session_id, &request.client_confirmation)
    {
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
    let mut identity = state.identity.lock().await;
    identity.credential = Some(result.credential);
    identity.pairing_code = None;
    identity.pairing_created_at = 0;
    if let Err(error_value) = state.store.save(&identity) {
        tracing::error!(error = %error_value, "failed to persist paired credential");
        identity.credential = None;
        return error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::Unspecified,
            "Could not save pairing state.",
        );
    }
    tracing::info!(host_id = %identity.host_id, "pairing succeeded");
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
            session_id: request.session_id,
        },
    )
}

async fn shutdown(State(state): State<Arc<AppState>>, headers: HeaderMap, body: Bytes) -> Response {
    if !is_protobuf(&headers) {
        return error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ErrorCode::MalformedRequest,
            "Content-Type must be application/x-protobuf.",
        );
    }
    let request = match ShutdownRequest::decode(body.clone()) {
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
    if let Err(reason) = authenticate(&state, &headers, "POST", "/v1/shutdown", &body).await {
        return unauthorized(reason);
    }
    match state.power.shutdown().await {
        Ok(()) => protobuf(StatusCode::ACCEPTED, ShutdownResponse { accepted: true }),
        Err(error_value) => {
            tracing::error!(error = %error_value, "local shutdown API rejected request");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ErrorCode::ShutdownFailed,
                "Windows rejected the shutdown request.",
            )
        }
    }
}

async fn authenticate(
    state: &AppState,
    headers: &HeaderMap,
    method: &str,
    path: &str,
    body: &[u8],
) -> Result<(), AuthError> {
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
        .await
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
        "DeckyPowerHost and the Decky plugin use incompatible protocol versions.",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        auth::{random_nonce, sign},
        power::MockPowerController,
        storage::DevelopmentStore,
    };
    use axum::{body::Body, http::Request};
    use tempfile::tempdir;
    use tower::ServiceExt;
    use uuid::Uuid;

    fn state(credential: Option<[u8; 32]>) -> Arc<AppState> {
        let path = tempdir().unwrap().keep().join("identity.json");
        Arc::new(AppState {
            identity: Mutex::new(HostIdentity {
                host_id: Uuid::nil(),
                credential,
                pairing_code: Some("123456".into()),
                pairing_created_at: now_unix(),
            }),
            pairing: Mutex::new(PairingCode::generate()),
            authenticator: Authenticator::default(),
            power: Arc::new(MockPowerController::default()),
            store: Arc::new(DevelopmentStore { path }),
            hostname: "test-pc".into(),
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
        let app = router(state(Some(secret)));
        let request = authenticated_request(
            "/v1/status",
            StatusRequest {
                protocol_version: 1,
            }
            .encode_to_vec(),
            &secret,
        );
        assert_eq!(app.oneshot(request).await.unwrap().status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn shutdown_requires_authentication() {
        let app = router(state(Some([8; 32])));
        let request = Request::post("/v1/shutdown")
            .header("content-type", CONTENT_TYPE)
            .body(Body::from(
                ShutdownRequest {
                    protocol_version: 1,
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
}
