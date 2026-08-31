use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use thiserror::Error;

use crate::DEFAULT_PORT;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostConfig {
    pub port: u16,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    port: Option<i64>,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("could not read configuration {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("malformed TOML in {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("port must be in 1..=65535, got {0}")]
    InvalidPort(i64),
    #[error("could not determine executable directory: {0}")]
    Executable(std::io::Error),
}

impl HostConfig {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            return Ok(Self { port: DEFAULT_PORT });
        }
        let text = fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_owned(),
            source,
        })?;
        let raw: RawConfig = toml::from_str(&text).map_err(|source| ConfigError::Parse {
            path: path.to_owned(),
            source,
        })?;
        let value = raw.port.unwrap_or(i64::from(DEFAULT_PORT));
        let port = u16::try_from(value)
            .ok()
            .filter(|port| *port != 0)
            .ok_or(ConfigError::InvalidPort(value))?;
        Ok(Self { port })
    }

    pub fn next_to_executable() -> Result<PathBuf, ConfigError> {
        let executable = std::env::current_exe().map_err(ConfigError::Executable)?;
        Ok(executable.with_file_name("DeckyMyRigHost.toml"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn load(contents: Option<&str>) -> Result<HostConfig, ConfigError> {
        let dir = tempdir().unwrap();
        let path = dir.path().join("DeckyMyRigHost.toml");
        if let Some(contents) = contents {
            fs::write(&path, contents).unwrap();
        }
        HostConfig::load(&path)
    }

    #[test]
    fn absent_file_uses_default() {
        assert_eq!(load(None).unwrap().port, DEFAULT_PORT);
    }
    #[test]
    fn missing_port_uses_default() {
        assert_eq!(load(Some("")).unwrap().port, DEFAULT_PORT);
    }
    #[test]
    fn custom_and_boundary_ports_work() {
        assert_eq!(load(Some("port = 48100")).unwrap().port, 48_100);
        assert_eq!(load(Some("port = 1")).unwrap().port, 1);
        assert_eq!(load(Some("port = 65535")).unwrap().port, 65_535);
    }
    #[test]
    fn invalid_ports_fail() {
        assert!(matches!(
            load(Some("port = 0")),
            Err(ConfigError::InvalidPort(0))
        ));
        assert!(matches!(
            load(Some("port = 65536")),
            Err(ConfigError::InvalidPort(65_536))
        ));
    }
    #[test]
    fn malformed_and_non_numeric_fail() {
        assert!(matches!(
            load(Some("port = [")),
            Err(ConfigError::Parse { .. })
        ));
        assert!(matches!(
            load(Some("port = \"x\"")),
            Err(ConfigError::Parse { .. })
        ));
    }
}
