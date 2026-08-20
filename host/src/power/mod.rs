use async_trait::async_trait;
use thiserror::Error;

#[cfg(windows)]
pub mod windows;

#[derive(Debug, Error)]
#[error("{message}")]
pub struct PowerError {
    pub message: String,
}

#[async_trait]
pub trait PowerController: Send + Sync {
    async fn shutdown(&self) -> Result<(), PowerError>;
}

#[derive(Default)]
pub struct MockPowerController {
    requested: std::sync::atomic::AtomicBool,
    fail: bool,
}

impl MockPowerController {
    pub fn failing() -> Self {
        Self {
            requested: false.into(),
            fail: true,
        }
    }
    pub fn was_requested(&self) -> bool {
        self.requested.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[async_trait]
impl PowerController for MockPowerController {
    async fn shutdown(&self) -> Result<(), PowerError> {
        self.requested
            .store(true, std::sync::atomic::Ordering::SeqCst);
        tracing::info!(
            "Authenticated shutdown requested. Mock mode enabled: no system shutdown performed."
        );
        if self.fail {
            Err(PowerError {
                message: "mock shutdown rejected".into(),
            })
        } else {
            Ok(())
        }
    }
}
