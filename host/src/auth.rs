use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::sync::Mutex;

const DOMAIN: &[u8] = b"deckypower-auth-v1\0";
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
        {
            let mut failures = self.failures.lock().await;
            while failures
                .front()
                .is_some_and(|seen| now.saturating_sub(*seen) > CLOCK_WINDOW.as_secs())
            {
                failures.pop_front();
            }
            if failures.len() >= 20 {
                return Err(AuthError::RateLimited);
            }
        }
        if now.abs_diff(request.timestamp) > CLOCK_WINDOW.as_secs() {
            self.record_failure(now).await;
            return Err(AuthError::Stale);
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
                self.record_failure(now).await;
                return Err(error);
            }
        };
        mac.update(&message);
        if mac.verify_slice(request.signature).is_err() {
            self.record_failure(now).await;
            return Err(AuthError::Invalid);
        }
        let mut nonces = self.accepted_nonces.lock().await;
        nonces.retain(|_, seen| now.saturating_sub(*seen) <= CLOCK_WINDOW.as_secs());
        if nonces.insert(request.nonce.to_vec(), now).is_some() {
            return Err(AuthError::Replay);
        }
        Ok(())
    }

    async fn record_failure(&self, now: u64) {
        self.failures.lock().await.push_back(now);
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
    }
}
