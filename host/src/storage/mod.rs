use std::{fs, io, path::PathBuf};

use rand::Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(windows)]
pub mod windows;

#[derive(Clone, Serialize, Deserialize)]
pub struct AcceptedShutdownNonce {
    pub timestamp: u64,
    pub nonce: [u8; 16],
}

#[derive(Clone, Serialize, Deserialize)]
pub struct HostIdentity {
    pub host_id: Uuid,
    pub credential: Option<[u8; 32]>,
    #[serde(default)]
    pub pairing_code: Option<String>,
    #[serde(default)]
    pub pairing_created_at: u64,
    #[serde(default)]
    pub accepted_shutdown_nonces: Vec<AcceptedShutdownNonce>,
    /// Last strict semantic version observed on an authenticated plugin request.
    /// This is informational and never participates in pairing authorization.
    #[serde(default)]
    pub last_client_version: Option<String>,
}

impl Default for HostIdentity {
    fn default() -> Self {
        Self {
            host_id: Uuid::new_v4(),
            credential: None,
            pairing_code: Some(format!("{:06}", rand::rng().random_range(0..1_000_000))),
            pairing_created_at: crate::auth::now_unix(),
            accepted_shutdown_nonces: Vec::new(),
            last_client_version: None,
        }
    }
}

pub trait CredentialStore: Send + Sync {
    fn load_or_create(&self) -> io::Result<HostIdentity>;
    fn save(&self, identity: &HostIdentity) -> io::Result<()>;
}

pub struct DevelopmentStore {
    pub path: PathBuf,
}

impl CredentialStore for DevelopmentStore {
    fn load_or_create(&self) -> io::Result<HostIdentity> {
        if !self.path.exists() {
            let value = HostIdentity::default();
            self.save(&value)?;
            return Ok(value);
        }
        let bytes = fs::read(&self.path)?;
        serde_json::from_slice(&bytes).map_err(io::Error::other)
    }
    fn save(&self, identity: &HostIdentity) -> io::Result<()> {
        let bytes = serde_json::to_vec(identity).map_err(io::Error::other)?;
        fs::write(&self.path, bytes)
    }
}
