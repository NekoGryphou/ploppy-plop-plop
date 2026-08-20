use std::time::{Duration, Instant};

use chacha20poly1305::{
    ChaCha20Poly1305, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use rand::{Rng, RngCore};
use sha2::Sha256;
use spake2::{Ed25519Group, Identity, Password, Spake2};
use thiserror::Error;

const PAIRING_LIFETIME: Duration = Duration::from_secs(300);
const MAX_ATTEMPTS: u8 = 5;
const CLIENT_ID: &[u8] = b"decky-client";
const HOST_ID: &[u8] = b"decky-host";
const CREDENTIAL_INFO: &[u8] = b"deckypower-pairing-credential-v1";
const CONFIRMATION_DOMAIN: &[u8] = b"deckypower-pairing-confirm-v1\0";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PairingError {
    #[error("pairing code expired")]
    Expired,
    #[error("too many pairing attempts")]
    RateLimited,
    #[error("invalid pairing exchange")]
    Invalid,
    #[error("credential encryption failed")]
    Encryption,
}

pub struct PairingCode {
    code: String,
    created: Instant,
    attempts: u8,
    pending: Option<PendingPairing>,
}

struct PendingPairing {
    session_id: [u8; 16],
    shared: Vec<u8>,
    client_message: Vec<u8>,
    host_message: Vec<u8>,
    created: Instant,
}

pub struct PairingStart {
    pub host_message: Vec<u8>,
    pub session_id: [u8; 16],
}

pub struct PairingResult {
    pub host_message: Vec<u8>,
    pub nonce: [u8; 12],
    pub encrypted_credential: Vec<u8>,
    pub credential: [u8; 32],
}

impl PairingCode {
    pub fn generate() -> Self {
        let value = rand::rng().random_range(0..1_000_000);
        Self {
            code: format!("{value:06}"),
            created: Instant::now(),
            attempts: 0,
            pending: None,
        }
    }

    pub fn from_code(code: String) -> Result<Self, PairingError> {
        Self::from_code_with_age(code, Duration::ZERO)
    }

    pub fn from_code_with_age(code: String, age: Duration) -> Result<Self, PairingError> {
        if code.len() != 6 || !code.bytes().all(|value| value.is_ascii_digit()) {
            return Err(PairingError::Invalid);
        }
        Ok(Self {
            code,
            created: Instant::now() - age.min(PAIRING_LIFETIME + Duration::from_secs(1)),
            attempts: 0,
            pending: None,
        })
    }

    pub fn display_code(&self) -> &str {
        &self.code
    }

    pub fn start(&mut self, client_message: &[u8]) -> Result<PairingStart, PairingError> {
        if self.created.elapsed() > PAIRING_LIFETIME {
            return Err(PairingError::Expired);
        }
        if self.attempts >= MAX_ATTEMPTS {
            return Err(PairingError::RateLimited);
        }
        self.attempts += 1;
        let (state, host_message) = Spake2::<Ed25519Group>::start_b(
            &Password::new(self.code.as_bytes()),
            &Identity::new(CLIENT_ID),
            &Identity::new(HOST_ID),
        );
        let shared = state
            .finish(client_message)
            .map_err(|_| PairingError::Invalid)?;
        let mut session_id = [0_u8; 16];
        rand::rng().fill_bytes(&mut session_id);
        self.pending = Some(PendingPairing {
            session_id,
            shared,
            client_message: client_message.to_vec(),
            host_message: host_message.clone(),
            created: Instant::now(),
        });
        Ok(PairingStart {
            host_message,
            session_id,
        })
    }

    pub fn finish(
        &mut self,
        session_id: &[u8],
        confirmation: &[u8],
    ) -> Result<PairingResult, PairingError> {
        let pending = self.pending.take().ok_or(PairingError::Invalid)?;
        if pending.created.elapsed() > PAIRING_LIFETIME || session_id != pending.session_id {
            return Err(PairingError::Invalid);
        }
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&pending.shared)
            .map_err(|_| PairingError::Invalid)?;
        mac.update(CONFIRMATION_DOMAIN);
        mac.update(&pending.client_message);
        mac.update(&pending.host_message);
        mac.update(&pending.session_id);
        mac.verify_slice(confirmation)
            .map_err(|_| PairingError::Invalid)?;
        let mut key = [0_u8; 32];
        Hkdf::<Sha256>::new(None, &pending.shared)
            .expand(CREDENTIAL_INFO, &mut key)
            .map_err(|_| PairingError::Encryption)?;
        let mut credential = [0_u8; 32];
        rand::rng().fill_bytes(&mut credential);
        let mut nonce = [0_u8; 12];
        rand::rng().fill_bytes(&mut nonce);
        let encrypted_credential = ChaCha20Poly1305::new((&key).into())
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &credential,
                    aad: &pending.host_message,
                },
            )
            .map_err(|_| PairingError::Encryption)?;
        self.created = Instant::now() - PAIRING_LIFETIME - Duration::from_secs(1);
        Ok(PairingResult {
            host_message: pending.host_message,
            nonce,
            encrypted_credential,
            credential,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successful_pairing_produces_256_bit_credential_and_invalidates_code() {
        let mut pairing = PairingCode {
            code: "123456".into(),
            created: Instant::now(),
            attempts: 0,
            pending: None,
        };
        let (client, message) = Spake2::<Ed25519Group>::start_a(
            &Password::new(b"123456"),
            &Identity::new(CLIENT_ID),
            &Identity::new(HOST_ID),
        );
        let started = pairing.start(&message).unwrap();
        let shared = client.finish(&started.host_message).unwrap();
        let mut confirmation = <Hmac<Sha256> as Mac>::new_from_slice(&shared).unwrap();
        confirmation.update(CONFIRMATION_DOMAIN);
        confirmation.update(&message);
        confirmation.update(&started.host_message);
        confirmation.update(&started.session_id);
        let result = pairing
            .finish(&started.session_id, &confirmation.finalize().into_bytes())
            .unwrap();
        let mut key = [0; 32];
        Hkdf::<Sha256>::new(None, &shared)
            .expand(CREDENTIAL_INFO, &mut key)
            .unwrap();
        let decrypted = ChaCha20Poly1305::new((&key).into())
            .decrypt(
                Nonce::from_slice(&result.nonce),
                Payload {
                    msg: &result.encrypted_credential,
                    aad: &result.host_message,
                },
            )
            .unwrap();
        assert_eq!(decrypted, result.credential);
        assert_eq!(decrypted.len(), 32);
        assert!(matches!(
            pairing.start(&message),
            Err(PairingError::Expired)
        ));
    }

    #[test]
    fn attempts_are_limited() {
        let mut pairing = PairingCode {
            code: "123456".into(),
            created: Instant::now(),
            attempts: MAX_ATTEMPTS,
            pending: None,
        };
        assert!(matches!(
            pairing.start(b"bad"),
            Err(PairingError::RateLimited)
        ));
    }

    #[test]
    fn persisted_expired_code_does_not_revive() {
        let mut pairing = PairingCode::from_code_with_age(
            "123456".into(),
            PAIRING_LIFETIME + Duration::from_secs(1),
        )
        .unwrap();
        assert!(matches!(
            pairing.start(b"message"),
            Err(PairingError::Expired)
        ));
    }

    #[test]
    fn wrong_code_confirmation_never_returns_a_credential() {
        let mut pairing = PairingCode::from_code("123456".into()).unwrap();
        let (client, message) = Spake2::<Ed25519Group>::start_a(
            &Password::new(b"999999"),
            &Identity::new(CLIENT_ID),
            &Identity::new(HOST_ID),
        );
        let started = pairing.start(&message).unwrap();
        let wrong_shared = client.finish(&started.host_message).unwrap();
        let mut confirmation = <Hmac<Sha256> as Mac>::new_from_slice(&wrong_shared).unwrap();
        confirmation.update(CONFIRMATION_DOMAIN);
        confirmation.update(&message);
        confirmation.update(&started.host_message);
        confirmation.update(&started.session_id);
        assert!(matches!(
            pairing.finish(&started.session_id, &confirmation.finalize().into_bytes()),
            Err(PairingError::Invalid)
        ));
    }
}
