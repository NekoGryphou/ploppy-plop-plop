use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use hmac::{Hmac, KeyInit, Mac};
use rand::Rng;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::sync::Mutex;

const DOMAIN: &[u8] = b"deckymyrig-auth-v1\0";
const RESPONSE_DOMAIN: &[u8] = b"deckymyrig-response-v1\0";
pub const CLOCK_WINDOW: Duration = Duration::from_secs(60);
pub const NONCE_LENGTH: usize = 16;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthError {
    #[error("malformed authentication header")]
    Malformed,
    #[error("request timestamp is outside the accepted clock window")]
    Stale,
    #[error("request nonce was already used")]
    Replay,
    #[error("request authentication failed")]
    Invalid,
    #[error("too many authentication failures")]
    RateLimited,
    #[error("authentication state could not be persisted")]
    Persistence,
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn random_nonce() -> [u8; NONCE_LENGTH] {
    let mut nonce = [0_u8; NONCE_LENGTH];
    rand::rng().fill_bytes(&mut nonce);
    nonce
}

pub fn canonical_message(
    timestamp: u64,
    nonce: &[u8],
    method: &str,
    path: &str,
    body: &[u8],
) -> Result<Vec<u8>, AuthError> {
    if nonce.len() != NONCE_LENGTH || !method.is_ascii() || !path.is_ascii() {
        return Err(AuthError::Malformed);
    }
    let method = method.to_ascii_uppercase();
    let mut output = Vec::with_capacity(128);
    output.extend_from_slice(DOMAIN);
    output.extend_from_slice(&timestamp.to_be_bytes());
    append_field(&mut output, nonce)?;
    append_field(&mut output, method.as_bytes())?;
    append_field(&mut output, path.as_bytes())?;
    output.extend_from_slice(&Sha256::digest(body));
    Ok(output)
}

fn append_field(output: &mut Vec<u8>, field: &[u8]) -> Result<(), AuthError> {
    let length = u16::try_from(field.len()).map_err(|_| AuthError::Malformed)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(field);
    Ok(())
}

pub fn sign(
    secret: &[u8],
    timestamp: u64,
    nonce: &[u8],
    method: &str,
    path: &str,
    body: &[u8],
) -> Result<[u8; 32], AuthError> {
    let message = canonical_message(timestamp, nonce, method, path, body)?;
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).map_err(|_| AuthError::Malformed)?;
    mac.update(&message);
    Ok(mac.finalize().into_bytes().into())
}

pub fn sign_response(
    secret: &[u8],
    request_nonce: &[u8],
    path: &str,
    status: u16,
    body: &[u8],
) -> Result<[u8; 32], AuthError> {
    let message = response_message(request_nonce, path, status, body)?;
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).map_err(|_| AuthError::Malformed)?;
    mac.update(&message);
    Ok(mac.finalize().into_bytes().into())
}

fn response_message(
    request_nonce: &[u8],
    path: &str,
    status: u16,
    body: &[u8],
) -> Result<Vec<u8>, AuthError> {
    if request_nonce.len() != NONCE_LENGTH || !path.is_ascii() {
        return Err(AuthError::Malformed);
    }
    let mut message = Vec::with_capacity(96);
    message.extend_from_slice(RESPONSE_DOMAIN);
    append_field(&mut message, request_nonce)?;
    append_field(&mut message, path.as_bytes())?;
    message.extend_from_slice(&status.to_be_bytes());
    message.extend_from_slice(&Sha256::digest(body));
    Ok(message)
}

pub fn verify_response(
    secret: &[u8],
    request_nonce: &[u8],
    path: &str,
    status: u16,
    body: &[u8],
    signature: &[u8],
) -> Result<(), AuthError> {
    let message = response_message(request_nonce, path, status, body)?;
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).map_err(|_| AuthError::Malformed)?;
    mac.update(&message);
    mac.verify_slice(signature).map_err(|_| AuthError::Invalid)
}

pub struct Authenticator {
    accepted_nonces: Mutex<HashMap<Vec<u8>, u64>>,
    failures: Mutex<VecDeque<u64>>,
}

pub struct AuthenticatedRequest<'a> {
    pub timestamp: u64,
    pub nonce: &'a [u8],
    pub signature: &'a [u8],
    pub method: &'a str,
    pub path: &'a str,
    pub body: &'a [u8],
}

impl Default for Authenticator {
    fn default() -> Self {
        Self {
            accepted_nonces: Mutex::new(HashMap::new()),
            failures: Mutex::new(VecDeque::new()),
        }
    }
}

impl Authenticator {
    pub async fn verify(
        &self,
        secret: &[u8],
        request: AuthenticatedRequest<'_>,
        now: u64,
    ) -> Result<(), AuthError> {
        if now.abs_diff(request.timestamp) > CLOCK_WINDOW.as_secs() {
            return Err(self.failed(now, AuthError::Stale).await);
        }
        let mut mac = Hmac::<Sha256>::new_from_slice(secret).map_err(|_| AuthError::Malformed)?;
        let message = match canonical_message(
            request.timestamp,
            request.nonce,
            request.method,
            request.path,
            request.body,
        ) {
            Ok(value) => value,
            Err(error) => {
                return Err(self.failed(now, error).await);
            }
        };
        mac.update(&message);
        if mac.verify_slice(request.signature).is_err() {
            return Err(self.failed(now, AuthError::Invalid).await);
        }
        let mut nonces = self.accepted_nonces.lock().await;
        nonces.retain(|_, seen| now.saturating_sub(*seen) <= CLOCK_WINDOW.as_secs());
        if nonces.insert(request.nonce.to_vec(), now).is_some() {
            return Err(AuthError::Replay);
        }
        Ok(())
    }

    async fn failed(&self, now: u64, error: AuthError) -> AuthError {
        let mut failures = self.failures.lock().await;
        while failures
            .front()
            .is_some_and(|seen| now.saturating_sub(*seen) > CLOCK_WINDOW.as_secs())
        {
            failures.pop_front();
        }
        if failures.len() >= 20 {
            return AuthError::RateLimited;
        }
        failures.push_back(now);
        error
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request<'a>(
        timestamp: u64,
        nonce: &'a [u8],
        signature: &'a [u8],
        path: &'a str,
        body: &'a [u8],
    ) -> AuthenticatedRequest<'a> {
        AuthenticatedRequest {
            timestamp,
            nonce,
            signature,
            method: "POST",
            path,
            body,
        }
    }

    #[tokio::test]
    async fn verifies_valid_and_rejects_replay() {
        let auth = Authenticator::default();
        let secret = [7; 32];
        let nonce = [9; 16];
        let body = b"body";
        let signature = sign(&secret, 100, &nonce, "POST", "/v1/status", body).unwrap();
        auth.verify(
            &secret,
            request(100, &nonce, &signature, "/v1/status", body),
            100,
        )
        .await
        .unwrap();
        assert_eq!(
            auth.verify(
                &secret,
                request(100, &nonce, &signature, "/v1/status", body),
                100
            )
            .await,
            Err(AuthError::Replay)
        );
    }

    #[tokio::test]
    async fn rejects_modification_stale_and_malformed_values() {
        let auth = Authenticator::default();
        let secret = [3; 32];
        let nonce = [4; 16];
        let signature = sign(&secret, 100, &nonce, "POST", "/v1/status", b"a").unwrap();
        assert_eq!(
            auth.verify(
                &secret,
                request(100, &nonce, &signature, "/v1/status", b"b"),
                100
            )
            .await,
            Err(AuthError::Invalid)
        );
        assert_eq!(
            auth.verify(
                &secret,
                request(100, &nonce, &signature, "/v1/shutdown", b"a"),
                100
            )
            .await,
            Err(AuthError::Invalid)
        );
        assert_eq!(
            auth.verify(
                &secret,
                request(100, &nonce, &signature, "/v1/status", b"a"),
                200
            )
            .await,
            Err(AuthError::Stale)
        );
        assert!(sign(&secret, 100, &[1; 3], "POST", "/", b"").is_err());
    }

    #[tokio::test]
    async fn excessive_failures_are_rate_limited() {
        let auth = Authenticator::default();
        let secret = [3; 32];
        let nonce = [4; 16];
        let bad_signature = [0; 32];
        for _ in 0..20 {
            assert_eq!(
                auth.verify(
                    &secret,
                    request(100, &nonce, &bad_signature, "/v1/status", b"a"),
                    100
                )
                .await,
                Err(AuthError::Invalid)
            );
        }
        assert_eq!(
            auth.verify(
                &secret,
                request(100, &nonce, &bad_signature, "/v1/status", b"a"),
                100
            )
            .await,
            Err(AuthError::RateLimited)
        );

        let valid_nonce = [5; 16];
        let valid_signature = sign(&secret, 100, &valid_nonce, "POST", "/v1/status", b"a").unwrap();
        auth.verify(
            &secret,
            request(100, &valid_nonce, &valid_signature, "/v1/status", b"a"),
            100,
        )
        .await
        .unwrap();
    }

    #[test]
    fn response_authentication_binds_nonce_path_status_and_body() {
        let secret = [4; 32];
        let nonce = [5; NONCE_LENGTH];
        let signature = sign_response(&secret, &nonce, "/v1/status", 200, b"body").unwrap();
        verify_response(&secret, &nonce, "/v1/status", 200, b"body", &signature).unwrap();
        assert_eq!(
            verify_response(&secret, &nonce, "/v1/status", 200, b"changed", &signature),
            Err(AuthError::Invalid)
        );
        assert_eq!(
            verify_response(&secret, &nonce, "/v1/shutdown", 200, b"body", &signature),
            Err(AuthError::Invalid)
        );
    }
}
