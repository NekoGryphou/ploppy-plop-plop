#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::path::PathBuf;

#[cfg(windows)]
use std::fs::OpenOptions;
use std::sync::Arc;

use decky_power_host::{
    HOST_VERSION, PROTOCOL_VERSION,
    auth::Authenticator,
    config::HostConfig,
    pairing::PairingCode,
    power::MockPowerController,
    server::{self, AppState},
    storage::{CredentialStore, DevelopmentStore},
};
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arguments: Vec<String> = std::env::args().collect();
    let _log_guard = init_logging(&arguments)?;
    if arguments.iter().any(|arg| arg == "--service") {
        #[cfg(windows)]
        return decky_power_host::service::windows::dispatch();
        #[cfg(not(windows))]
        anyhow::bail!("service mode is only supported on Windows");
    }
    run_portable(arguments).await
}

async fn run_portable(arguments: Vec<String>) -> anyhow::Result<()> {
    if !arguments.iter().any(|arg| arg == "--dev") {
        anyhow::bail!("unknown arguments; use --dev --mock-shutdown for development");
    }
    if !arguments.iter().any(|arg| arg == "--mock-shutdown") {
        anyhow::bail!("development mode requires --mock-shutdown");
    }
    let config_path = config_override(&arguments).unwrap_or(HostConfig::next_to_executable()?);
    let config = HostConfig::load(&config_path)?;
    let listen_port = if arguments.iter().any(|arg| arg == "--ephemeral-port") {
        0
    } else {
        config.port
    };
    let state_path = config_path.with_file_name("DeckyPowerHost.dev-state.json");
    let store: Arc<dyn CredentialStore> = Arc::new(DevelopmentStore { path: state_path });
    let mut identity = store.load_or_create()?;
    let requested_code = arguments
        .windows(2)
        .find(|pair| pair[0] == "--pairing-code-value")
        .map(|pair| pair[1].clone());
    let pairing = if let Some(code) = requested_code {
        let pairing = PairingCode::from_code(code)?;
        identity.pairing_code = Some(pairing.display_code().to_owned());
        identity.pairing_created_at = decky_power_host::auth::now_unix();
        store.save(&identity)?;
        pairing
    } else if let Some(code) = identity.pairing_code.clone() {
        PairingCode::from_code_with_age(
            code,
            decky_power_host::management::persisted_code_age(
                identity.pairing_created_at,
                decky_power_host::auth::now_unix(),
            ),
        )?
    } else {
        PairingCode::generate()
    };
    println!("DeckyPowerHost pairing code: {}", pairing.display_code());
    tracing::info!(version = HOST_VERSION, protocol_version = PROTOCOL_VERSION, config = %config_path.display(), port = config.port, "DeckyPowerHost starting in safe development mode");
    let listener = server::bind(listen_port)
        .await
        .map_err(|error| anyhow::anyhow!("could not listen on 0.0.0.0:{listen_port}: {error}"))?;
    println!("DECKY_POWER_LISTEN_PORT={}", listener.local_addr()?.port());
    let hostname = hostname::get()?.to_string_lossy().into_owned();
    let latest_client_version = identity.last_client_version.clone();
    server::serve(
        listener,
        Arc::new(AppState {
            identity: Mutex::new(identity),
            pairing: Mutex::new(pairing),
            authenticator: Authenticator::default(),
            power: Arc::new(MockPowerController::default()),
            store,
            hostname,
            latest_client_version: Mutex::new(latest_client_version),
        }),
    )
    .await?;
    Ok(())
}

fn init_logging(
    arguments: &[String],
) -> anyhow::Result<Option<tracing_appender::non_blocking::WorkerGuard>> {
    let filter = tracing_subscriber::EnvFilter::from_default_env()
        .add_directive("decky_power_host=info".parse()?);
    #[cfg(windows)]
    if arguments.iter().any(|argument| argument == "--service") {
        let directory = std::env::var_os("ProgramData")
            .map(PathBuf::from)
            .ok_or_else(|| anyhow::anyhow!("ProgramData is unavailable"))?
            .join("DeckyPowerHost");
        let log_path = directory.join("DeckyPowerHost.log");
        rotate_log_if_needed(&log_path, 5 * 1024 * 1024)?;
        match std::fs::create_dir_all(&directory)
            .and_then(|_| OpenOptions::new().create(true).append(true).open(&log_path))
        {
            Ok(file) => {
                let (writer, guard) = tracing_appender::non_blocking(file);
                tracing_subscriber::fmt()
                    .with_env_filter(filter)
                    .with_ansi(false)
                    .with_writer(writer)
                    .init();
                return Ok(Some(guard));
            }
            Err(error) => {
                eprintln!(
                    "DeckyPowerHost could not open its service log at {}: {error}. Logging to stderr.",
                    log_path.display()
                );
            }
        }
    }
    let _ = arguments;
    tracing_subscriber::fmt().with_env_filter(filter).init();
    Ok(None)
}

#[cfg(any(windows, test))]
fn rotate_log_if_needed(path: &std::path::Path, maximum_bytes: u64) -> std::io::Result<()> {
    let Ok(metadata) = std::fs::metadata(path) else {
        return Ok(());
    };
    if metadata.len() < maximum_bytes {
        return Ok(());
    }
    let archived = path.with_extension("log.1");
    if archived.exists() {
        std::fs::remove_file(&archived)?;
    }
    std::fs::rename(path, archived)
}

fn config_override(arguments: &[String]) -> Option<PathBuf> {
    arguments
        .windows(2)
        .find(|pair| pair[0] == "--config")
        .map(|pair| PathBuf::from(&pair[1]))
}

#[cfg(test)]
mod logging_tests {
    use super::rotate_log_if_needed;

    #[test]
    fn service_log_rotation_is_bounded_to_one_archive() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("DeckyPowerHost.log");
        std::fs::write(&path, b"old log").unwrap();
        std::fs::write(path.with_extension("log.1"), b"older log").unwrap();

        rotate_log_if_needed(&path, 4).unwrap();

        assert!(!path.exists());
        assert_eq!(
            std::fs::read(path.with_extension("log.1")).unwrap(),
            b"old log"
        );
    }
}
