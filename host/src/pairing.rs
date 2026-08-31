use std::{
    collections::HashMap,
    net::IpAddr,
    time::{Duration, Instant},
};

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
const MAX_CONFIRMATION_FAILURES: u8 = 5;
const MAX_PENDING_SESSIONS: usize = 128;
const MAX_PENDING_SESSIONS_PER_SOURCE: usize = 8;
const MAX_FAILURE_SOURCES: usize = 256;
const CLIENT_ID: &[u8] = b"decky-client";
const HOST_ID: &[u8] = b"decky-host";
const CREDENTIAL_INFO: &[u8] = b"deckymyrig-pairing-credential-v1";
const CONFIRMATION_DOMAIN: &[u8] = b"deckymyrig-pairing-confirm-v1\0";
const CREDENTIAL_AAD_DOMAIN: &[u8] = b"deckymyrig-pairing-credential-aad-v1\0";

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
    confirmation_failures: HashMap<Option<IpAddr>, u8>,
    pending: HashMap<[u8; 16], PendingPairing>,
    completed: HashMap<[u8; 16], CompletedPairing>,
}

struct PendingPairing {
    session_id: [u8; 16],
    shared: Vec<u8>,
    client_message: Vec<u8>,
    host_message: Vec<u8>,
    created: Instant,
    source: Option<IpAddr>,
}

struct CompletedPairing {
    confirmation: Vec<u8>,
    result: PairingResult,
    created: Instant,
}

pub struct PairingStart {
    pub host_message: Vec<u8>,
    pub session_id: [u8; 16],
}

#[derive(Clone)]
pub struct PairingResult {
    pub host_message: Vec<u8>,
    pub nonce: [u8; 12],
    pub encrypted_credential: Vec<u8>,
    pub credential: [u8; 32],
}

pub struct CredentialContext<'a> {
    pub hostname: &'a str,
    pub host_version: &'a str,
    pub protocol_version: u32,
    pub host_id: &'a str,
}

pub fn credential_aad(
    host_message: &[u8],
    session_id: &[u8],
    context: &CredentialContext<'_>,
) -> Result<Vec<u8>, PairingError> {
    let mut aad = CREDENTIAL_AAD_DOMAIN.to_vec();
    for field in [
        host_message,
        session_id,
        context.hostname.as_bytes(),
        context.host_version.as_bytes(),
        context.host_id.as_bytes(),
    ] {
        let length = u16::try_from(field.len()).map_err(|_| PairingError::Invalid)?;
        aad.extend_from_slice(&length.to_be_bytes());
        aad.extend_from_slice(field);
    }
    aad.extend_from_slice(&context.protocol_version.to_be_bytes());
    Ok(aad)
}

impl PairingCode {
    pub fn generate() -> Self {
        let value = rand::rng().random_range(0..1_000_000);
        Self {
            code: format!("{value:06}"),
            created: Instant::now(),
            confirmation_failures: HashMap::new(),
            pending: HashMap::new(),
            completed: HashMap::new(),
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
            confirmation_failures: HashMap::new(),
            pending: HashMap::new(),
            completed: HashMap::new(),
        })
    }

    pub fn display_code(&self) -> &str {
        &self.code
    }

    pub fn expires_in(&self) -> Duration {
        PAIRING_LIFETIME.saturating_sub(self.created.elapsed())
    }

    pub fn regenerate(&mut self) {
        let previous = self.code.clone();
        loop {
            *self = Self::generate();
            if self.code != previous {
                break;
            }
        }
    }

    pub fn start(&mut self, client_message: &[u8]) -> Result<PairingStart, PairingError> {
        self.start_for_source(client_message, None)
    }

    pub fn start_for_source(
        &mut self,
        client_message: &[u8],
        source: Option<IpAddr>,
    ) -> Result<PairingStart, PairingError> {
        if self.created.elapsed() > PAIRING_LIFETIME {
            return Err(PairingError::Expired);
        }
        self.prune_sessions();
        if self
            .confirmation_failures
            .get(&source)
            .copied()
            .unwrap_or(0)
            >= MAX_CONFIRMATION_FAILURES
            || !self.confirmation_failures.contains_key(&source)
                && self.confirmation_failures.len() >= MAX_FAILURE_SOURCES
            || self.pending.len() >= MAX_PENDING_SESSIONS
            || source.is_some()
                && self
                    .pending
                    .values()
                    .filter(|pending| pending.source == source)
                    .count()
                    >= MAX_PENDING_SESSIONS_PER_SOURCE
        {
            return Err(PairingError::RateLimited);
        }
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
        self.pending.insert(
            session_id,
            PendingPairing {
                session_id,
                shared,
                client_message: client_message.to_vec(),
                host_message: host_message.clone(),
                created: Instant::now(),
                source,
            },
        );
        Ok(PairingStart {
            host_message,
            session_id,
        })
    }

    pub fn finish(
        &mut self,
        session_id: &[u8],
        confirmation: &[u8],
        context: &CredentialContext<'_>,
    ) -> Result<PairingResult, PairingError> {
        let session_id: [u8; 16] = session_id.try_into().map_err(|_| PairingError::Invalid)?;
        self.prune_sessions();
        if let Some(completed) = self.completed.get(&session_id) {
            return if completed.confirmation == confirmation {
                Ok(completed.result.clone())
            } else {
                Err(PairingError::Invalid)
            };
        }
        let pending = self.pending.get(&session_id).ok_or(PairingError::Invalid)?;
        if pending.created.elapsed() > PAIRING_LIFETIME {
            return Err(PairingError::Invalid);
        }
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&pending.shared)
            .map_err(|_| PairingError::Invalid)?;
        mac.update(CONFIRMATION_DOMAIN);
        mac.update(&pending.client_message);
        mac.update(&pending.host_message);
        mac.update(&pending.session_id);
        if mac.verify_slice(confirmation).is_err() {
            let source = pending.source;
            self.pending.remove(&session_id);
            let failures = self.confirmation_failures.entry(source).or_default();
            *failures = failures.saturating_add(1);
            return Err(if *failures >= MAX_CONFIRMATION_FAILURES {
                PairingError::RateLimited
            } else {
                PairingError::Invalid
            });
        }
        let mut key = [0_u8; 32];
        Hkdf::<Sha256>::new(None, &pending.shared)
            .expand(CREDENTIAL_INFO, &mut key)
            .map_err(|_| PairingError::Encryption)?;
        let mut credential = [0_u8; 32];
        rand::rng().fill_bytes(&mut credential);
        let mut nonce = [0_u8; 12];
        rand::rng().fill_bytes(&mut nonce);
        let aad = credential_aad(&pending.host_message, &pending.session_id, context)?;
        let encrypted_credential = ChaCha20Poly1305::new((&key).into())
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &credential,
                    aad: &aad,
                },
            )
            .map_err(|_| PairingError::Encryption)?;
        Ok(PairingResult {
            host_message: pending.host_message.clone(),
            nonce,
            encrypted_credential,
            credential,
        })
    }

    pub fn is_completed(&self, session_id: &[u8]) -> bool {
        <[u8; 16]>::try_from(session_id)
            .ok()
            .is_some_and(|session_id| self.completed.contains_key(&session_id))
    }

    pub fn commit(
        &mut self,
        session_id: &[u8],
        confirmation: &[u8],
        result: PairingResult,
    ) -> Result<(), PairingError> {
        let session_id: [u8; 16] = session_id.try_into().map_err(|_| PairingError::Invalid)?;
        if !self.pending.contains_key(&session_id) {
            return Err(PairingError::Invalid);
        }
        self.pending.clear();
        self.completed.clear();
        self.completed.insert(
            session_id,
            CompletedPairing {
                confirmation: confirmation.to_vec(),
                result,
                created: Instant::now(),
            },
        );
        self.created = Instant::now() - PAIRING_LIFETIME - Duration::from_secs(1);
        Ok(())
    }

    fn prune_sessions(&mut self) {
        self.pending
            .retain(|_, pending| pending.created.elapsed() <= PAIRING_LIFETIME);
        self.completed
            .retain(|_, completed| completed.created.elapsed() <= PAIRING_LIFETIME);
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
            confirmation_failures: HashMap::new(),
            pending: HashMap::new(),
            completed: HashMap::new(),
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
        let confirmation = confirmation.finalize().into_bytes();
        let result = pairing
            .finish(
                &started.session_id,
                &confirmation,
                &CredentialContext {
                    hostname: "test-host",
                    host_version: "0.1.0",
                    protocol_version: 1,
                    host_id: "test-id",
                },
            )
            .unwrap();
        pairing
            .commit(&started.session_id, &confirmation, result.clone())
            .unwrap();
        let mut key = [0; 32];
        Hkdf::<Sha256>::new(None, &shared)
            .expand(CREDENTIAL_INFO, &mut key)
            .unwrap();
        let aad = credential_aad(
            &result.host_message,
            &started.session_id,
            &CredentialContext {
                hostname: "test-host",
                host_version: "0.1.0",
                protocol_version: 1,
                host_id: "test-id",
            },
        )
        .unwrap();
        let decrypted = ChaCha20Poly1305::new((&key).into())
            .decrypt(
                Nonce::from_slice(&result.nonce),
                Payload {
                    msg: &result.encrypted_credential,
                    aad: &aad,
                },
            )
            .unwrap();
        assert_eq!(decrypted, result.credential);
        assert_eq!(decrypted.len(), 32);
        let tampered_aad = credential_aad(
            &result.host_message,
            &started.session_id,
            &CredentialContext {
                hostname: "test-host",
                host_version: "0.1.0",
                protocol_version: 1,
                host_id: "tampered-id",
            },
        )
        .unwrap();
        assert!(
            ChaCha20Poly1305::new((&key).into())
                .decrypt(
                    Nonce::from_slice(&result.nonce),
                    Payload {
                        msg: &result.encrypted_credential,
                        aad: &tampered_aad,
                    },
                )
                .is_err()
        );
        assert!(matches!(
            pairing.start(&message),
            Err(PairingError::Expired)
        ));
    }

    #[test]
    fn pairing_starts_are_limited_per_source_without_blocking_other_clients() {
        let mut pairing = PairingCode::from_code("123456".into()).unwrap();
        let hostile_source = "192.0.2.10".parse().unwrap();
        for _ in 0..MAX_PENDING_SESSIONS_PER_SOURCE {
            let (_, message) = Spake2::<Ed25519Group>::start_a(
                &Password::new(b"000000"),
                &Identity::new(CLIENT_ID),
                &Identity::new(HOST_ID),
            );
            assert!(
                pairing
                    .start_for_source(&message, Some(hostile_source))
                    .is_ok()
            );
        }
        let (_, message) = Spake2::<Ed25519Group>::start_a(
            &Password::new(b"000000"),
            &Identity::new(CLIENT_ID),
            &Identity::new(HOST_ID),
        );
        assert!(matches!(
            pairing.start_for_source(&message, Some(hostile_source)),
            Err(PairingError::RateLimited)
        ));
        assert!(
            pairing
                .start_for_source(&message, Some("192.0.2.11".parse().unwrap()))
                .is_ok()
        );
    }

    #[test]
    fn failed_confirmations_do_not_lock_out_a_different_source() {
        let mut pairing = PairingCode::from_code("123456".into()).unwrap();
        let hostile_source = "192.0.2.10".parse().unwrap();
        let context = CredentialContext {
            hostname: "test-host",
            host_version: "0.1.0",
            protocol_version: 1,
            host_id: "test-id",
        };
        for attempt in 0..MAX_CONFIRMATION_FAILURES {
            let (client, message) = Spake2::<Ed25519Group>::start_a(
                &Password::new(b"000000"),
                &Identity::new(CLIENT_ID),
                &Identity::new(HOST_ID),
            );
            let started = pairing
                .start_for_source(&message, Some(hostile_source))
                .unwrap();
            let shared = client.finish(&started.host_message).unwrap();
            let mut confirmation = <Hmac<Sha256> as Mac>::new_from_slice(&shared).unwrap();
            confirmation.update(CONFIRMATION_DOMAIN);
            confirmation.update(&message);
            confirmation.update(&started.host_message);
            confirmation.update(&started.session_id);
            let result = pairing.finish(
                &started.session_id,
                &confirmation.finalize().into_bytes(),
                &context,
            );
            if attempt + 1 == MAX_CONFIRMATION_FAILURES {
                assert!(matches!(result, Err(PairingError::RateLimited)));
            } else {
                assert!(matches!(result, Err(PairingError::Invalid)));
            }
        }

        let (_, message) = Spake2::<Ed25519Group>::start_a(
            &Password::new(b"123456"),
            &Identity::new(CLIENT_ID),
            &Identity::new(HOST_ID),
        );
        assert!(
            pairing
                .start_for_source(&message, Some("192.0.2.11".parse().unwrap()))
                .is_ok()
        );
    }

    #[test]
    fn concurrent_pairing_starts_do_not_invalidate_existing_sessions() {
        let mut pairing = PairingCode::from_code("123456".into()).unwrap();
        let (first_client, first_message) = Spake2::<Ed25519Group>::start_a(
            &Password::new(b"123456"),
            &Identity::new(CLIENT_ID),
            &Identity::new(HOST_ID),
        );
        let first = pairing.start(&first_message).unwrap();
        let (_, second_message) = Spake2::<Ed25519Group>::start_a(
            &Password::new(b"123456"),
            &Identity::new(CLIENT_ID),
            &Identity::new(HOST_ID),
        );
        pairing.start(&second_message).unwrap();

        let shared = first_client.finish(&first.host_message).unwrap();
        let mut confirmation = <Hmac<Sha256> as Mac>::new_from_slice(&shared).unwrap();
        confirmation.update(CONFIRMATION_DOMAIN);
        confirmation.update(&first_message);
        confirmation.update(&first.host_message);
        confirmation.update(&first.session_id);

        assert!(
            pairing
                .finish(
                    &first.session_id,
                    &confirmation.finalize().into_bytes(),
                    &CredentialContext {
                        hostname: "test-host",
                        host_version: "0.1.0",
                        protocol_version: 1,
                        host_id: "test-id",
                    },
                )
                .is_ok()
        );
    }

    #[test]
    fn prepared_pairing_remains_retryable_until_persistence_is_committed() {
        let mut pairing = PairingCode::from_code("123456".into()).unwrap();
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
        let confirmation = confirmation.finalize().into_bytes();
        let context = CredentialContext {
            hostname: "test-host",
            host_version: "0.1.0",
            protocol_version: 1,
            host_id: "test-id",
        };

        assert!(
            pairing
                .finish(&started.session_id, &confirmation, &context)
                .is_ok()
        );
        assert!(
            pairing
                .finish(&started.session_id, &confirmation, &context)
                .is_ok()
        );
        assert!(pairing.expires_in() > Duration::from_secs(299));
    }

    #[test]
    fn committed_pairing_retries_are_identical_and_reject_tampering() {
        let mut pairing = PairingCode::from_code("123456".into()).unwrap();
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
        let confirmation = confirmation.finalize().into_bytes();
        let context = CredentialContext {
            hostname: "test-host",
            host_version: "0.1.0",
            protocol_version: 1,
            host_id: "test-id",
        };
        let result = pairing
            .finish(&started.session_id, &confirmation, &context)
            .unwrap();
        pairing
            .commit(&started.session_id, &confirmation, result.clone())
            .unwrap();

        let retry = pairing
            .finish(&started.session_id, &confirmation, &context)
            .unwrap();
        assert_eq!(retry.credential, result.credential);
        assert_eq!(retry.encrypted_credential, result.encrypted_credential);
        assert!(matches!(
            pairing.finish(&started.session_id, b"tampered", &context),
            Err(PairingError::Invalid)
        ));
    }

    #[test]
    fn completed_retry_and_abandoned_pending_sessions_expire() {
        let mut pairing = PairingCode::from_code("123456".into()).unwrap();
        let source = Some("192.0.2.10".parse().unwrap());
        for _ in 0..MAX_PENDING_SESSIONS_PER_SOURCE {
            let (_, message) = Spake2::<Ed25519Group>::start_a(
                &Password::new(b"123456"),
                &Identity::new(CLIENT_ID),
                &Identity::new(HOST_ID),
            );
            pairing.start_for_source(&message, source).unwrap();
        }
        for pending in pairing.pending.values_mut() {
            pending.created = Instant::now() - PAIRING_LIFETIME - Duration::from_secs(1);
        }
        let (_, message) = Spake2::<Ed25519Group>::start_a(
            &Password::new(b"123456"),
            &Identity::new(CLIENT_ID),
            &Identity::new(HOST_ID),
        );
        assert!(pairing.start_for_source(&message, source).is_ok());

        pairing.pending.clear();
        pairing.completed.insert(
            [7; 16],
            CompletedPairing {
                confirmation: b"confirmation".to_vec(),
                result: PairingResult {
                    host_message: Vec::new(),
                    nonce: [0; 12],
                    encrypted_credential: Vec::new(),
                    credential: [0; 32],
                },
                created: Instant::now() - PAIRING_LIFETIME - Duration::from_secs(1),
            },
        );
        assert!(matches!(
            pairing.finish(
                &[7; 16],
                b"confirmation",
                &CredentialContext {
                    hostname: "test-host",
                    host_version: "0.1.0",
                    protocol_version: 1,
                    host_id: "test-id",
                }
            ),
            Err(PairingError::Invalid)
        ));
    }

    #[test]
    fn regeneration_invalidates_an_exchange_that_was_already_started() {
        let mut pairing = PairingCode::from_code("123456".into()).unwrap();
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
        pairing.regenerate();

        assert!(matches!(
            pairing.finish(
                &started.session_id,
                &confirmation.finalize().into_bytes(),
                &CredentialContext {
                    hostname: "test-host",
                    host_version: "0.1.0",
                    protocol_version: 1,
                    host_id: "test-id",
                },
            ),
            Err(PairingError::Invalid)
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
            pairing.finish(
                &started.session_id,
                &confirmation.finalize().into_bytes(),
                &CredentialContext {
                    hostname: "test-host",
                    host_version: "0.1.0",
                    protocol_version: 1,
                    host_id: "test-id",
                },
            ),
            Err(PairingError::Invalid)
        ));
    }

    #[test]
    fn regeneration_invalidates_the_old_code_and_resets_expiration() {
        let mut pairing = PairingCode::from_code("123456".into()).unwrap();
        pairing.regenerate();

        assert_ne!(pairing.display_code(), "123456");
        assert!(pairing.expires_in() > Duration::from_secs(299));

        let (client, message) = Spake2::<Ed25519Group>::start_a(
            &Password::new(b"123456"),
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
            pairing.finish(
                &started.session_id,
                &confirmation.finalize().into_bytes(),
                &CredentialContext {
                    hostname: "test-host",
                    host_version: "0.1.0",
                    protocol_version: 1,
                    host_id: "test-id",
                },
            ),
            Err(PairingError::Invalid)
        ));
    }
}
