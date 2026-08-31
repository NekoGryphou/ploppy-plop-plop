use std::{io, time::Duration};

use crate::{auth::now_unix, pairing::PairingCode, server::AppState};

pub fn persisted_code_age(created_at: u64, now: u64) -> Duration {
    if created_at == 0 || created_at > now {
        Duration::from_secs(301)
    } else {
        Duration::from_secs(now - created_at)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingState {
    pub paired: bool,
    pub code: Option<String>,
    pub expires_in: Duration,
}

pub async fn pairing_state(state: &AppState) -> PairingState {
    let pairing = state.pairing.lock().await;
    let identity = state.identity.lock().await;
    PairingState {
        paired: identity.credential.is_some(),
        code: identity
            .pairing_code
            .as_ref()
            .map(|_| pairing.display_code().to_owned()),
        expires_in: pairing.expires_in(),
    }
}

pub async fn generate_pairing_code(state: &AppState) -> io::Result<PairingState> {
    let mut pairing = state.pairing.lock().await;
    let mut identity = state.identity.lock().await;
    let generated = PairingCode::generate();
    let mut updated = identity.clone();
    updated.pairing_code = Some(generated.display_code().to_owned());
    updated.pairing_created_at = now_unix();
    state.store.save(&updated)?;
    *identity = updated;
    *pairing = generated;
    Ok(PairingState {
        paired: identity.credential.is_some(),
        code: identity.pairing_code.clone(),
        expires_in: pairing.expires_in(),
    })
}

#[cfg(any(test, feature = "test-management"))]
pub async fn state_for_test(
    store: std::sync::Arc<dyn crate::storage::CredentialStore>,
    hostname: &str,
) -> io::Result<std::sync::Arc<AppState>> {
    let identity = store.load_or_create()?;
    let age = persisted_code_age(identity.pairing_created_at, crate::auth::now_unix());
    let pairing = identity
        .pairing_code
        .clone()
        .map(|code| PairingCode::from_code_with_age(code, age))
        .transpose()
        .map_err(io::Error::other)?
        .unwrap_or_else(PairingCode::generate);
    let latest_client_version = identity.last_client_version.clone();
    Ok(std::sync::Arc::new(AppState {
        identity: tokio::sync::Mutex::new(identity),
        pairing: tokio::sync::Mutex::new(pairing),
        authenticator: crate::auth::Authenticator::default(),
        power: std::sync::Arc::new(crate::power::MockPowerController::default()),
        store,
        hostname: hostname.into(),
        latest_client_version: tokio::sync::Mutex::new(latest_client_version),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{CredentialStore, DevelopmentStore};
    use std::sync::Arc;
    use tempfile::tempdir;

    #[tokio::test]
    async fn local_management_generates_persists_and_regenerates_one_code() {
        let path = tempdir().unwrap().keep().join("identity.json");
        let store: Arc<dyn CredentialStore> = Arc::new(DevelopmentStore { path: path.clone() });
        let state = state_for_test(store, "managed-host").await.unwrap();
        let first = generate_pairing_code(&state).await.unwrap();
        let second = generate_pairing_code(&state).await.unwrap();

        assert_eq!(first.code.as_ref().unwrap().len(), 6);
        assert_ne!(first.code, second.code);
        assert!(second.expires_in > Duration::from_secs(299));
        let persisted = DevelopmentStore { path }.load_or_create().unwrap();
        assert_eq!(persisted.pairing_code, second.code);
    }

    #[tokio::test]
    async fn restart_preserves_code_age_instead_of_reviving_it() {
        let path = tempdir().unwrap().keep().join("identity.json");
        let store = DevelopmentStore { path: path.clone() };
        let mut identity = store.load_or_create().unwrap();
        identity.pairing_code = Some("123456".into());
        identity.pairing_created_at = crate::auth::now_unix().saturating_sub(301);
        store.save(&identity).unwrap();

        let state = state_for_test(Arc::new(DevelopmentStore { path }), "restarted-host")
            .await
            .unwrap();
        let pairing = pairing_state(&state).await;

        assert_eq!(pairing.code.as_deref(), Some("123456"));
        assert_eq!(pairing.expires_in, Duration::ZERO);
    }

    #[tokio::test]
    async fn restart_restores_the_last_authenticated_plugin_version() {
        let path = tempdir().unwrap().keep().join("identity.json");
        let store = DevelopmentStore { path: path.clone() };
        let mut identity = store.load_or_create().unwrap();
        identity.last_client_version = Some("1.4.2".into());
        store.save(&identity).unwrap();

        let state = state_for_test(Arc::new(DevelopmentStore { path }), "restarted-host")
            .await
            .unwrap();

        assert_eq!(
            state.latest_client_version.lock().await.as_deref(),
            Some("1.4.2")
        );
    }

    #[tokio::test]
    async fn clock_rollback_does_not_revive_a_persisted_code() {
        let path = tempdir().unwrap().keep().join("identity.json");
        let store = DevelopmentStore { path: path.clone() };
        let mut identity = store.load_or_create().unwrap();
        identity.pairing_code = Some("123456".into());
        identity.pairing_created_at = crate::auth::now_unix() + 60;
        store.save(&identity).unwrap();

        let state = state_for_test(Arc::new(DevelopmentStore { path }), "rollback-host")
            .await
            .unwrap();
        assert_eq!(pairing_state(&state).await.expires_in, Duration::ZERO);
    }
}
