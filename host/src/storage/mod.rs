use std::{fs, io, path::PathBuf};

use rand::Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(windows)]
pub mod windows;

#[derive(Clone, Serialize, Deserialize)]
pub struct HostIdentity {
    pub host_id: Uuid,
    pub credential: Option<[u8; 32]>,
    #[serde(default)]
    pub pairing_code: Option<String>,
    #[serde(default)]
    pub pairing_created_at: u64,
}

impl Default for HostIdentity {
    fn default() -> Self {
        Self {
            host_id: Uuid::new_v4(),
            credential: None,
            pairing_code: Some(format!("{:06}", rand::rng().random_range(0..1_000_000))),
            pairing_created_at: crate::auth::now_unix(),
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
