#[cfg(windows)]
use rand::Rng;
use std::{path::PathBuf, sync::Arc};

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
    #[cfg(windows)]
    if arguments.iter().any(|arg| arg == "--pairing-code") {
        let store = decky_power_host::storage::windows::WindowsCredentialStore::program_data()?;
        let mut identity = store.load_or_create()?;
        if identity.credential.is_some() {
            println!("This host is already paired.");
            return Ok(());
        }
        identity.pairing_code = Some(format!("{:06}", rand::rng().random_range(0..1_000_000)));
        identity.pairing_created_at = decky_power_host::auth::now_unix();
        store.save(&identity)?;
        println!(
            "DeckyPowerHost pairing code: {}",
            identity.pairing_code.as_deref().unwrap_or("unavailable")
        );
        return Ok(());
    }
    #[cfg(windows)]
    if arguments.iter().any(|arg| arg == "--reset-pairing") {
        let store = decky_power_host::storage::windows::WindowsCredentialStore::program_data()?;
        let mut identity = store.load_or_create()?;
        identity.credential = None;
        identity.pairing_code = Some(format!("{:06}", rand::rng().random_range(0..1_000_000)));
        identity.pairing_created_at = decky_power_host::auth::now_unix();
        store.save(&identity)?;
        println!(
            "DeckyPowerHost pairing was reset. New pairing code: {}",
            identity.pairing_code.as_deref().unwrap_or("unavailable")
        );
        return Ok(());
    }
    if !arguments.iter().any(|arg| arg == "--dev") {
        #[cfg(windows)]
        return decky_power_host::service::windows::dispatch();
        #[cfg(not(windows))]
        anyhow::bail!("production mode is a Windows service; use --dev --mock-shutdown");
    }
    if !arguments.iter().any(|arg| arg == "--mock-shutdown") {
        anyhow::bail!("development mode requires --mock-shutdown");
    }
    let config_path = config_override(&arguments).unwrap_or(HostConfig::next_to_executable()?);
    let config = HostConfig::load(&config_path)?;
    let state_path = config_path.with_file_name("DeckyPowerHost.dev-state.json");
    let store: Arc<dyn CredentialStore> = Arc::new(DevelopmentStore { path: state_path });
    let identity = store.load_or_create()?;
    let requested_code = arguments
        .windows(2)
        .find(|pair| pair[0] == "--pairing-code-value")
        .map(|pair| pair[1].clone());
    let pairing = if let Some(code) = requested_code {
        PairingCode::from_code(code)?
    } else if let Some(code) = identity.pairing_code.clone() {
        PairingCode::from_code_with_age(
            code,
            std::time::Duration::from_secs(
                decky_power_host::auth::now_unix().saturating_sub(identity.pairing_created_at),
            ),
        )?
    } else {
        PairingCode::generate()
    };
    println!("DeckyPowerHost pairing code: {}", pairing.display_code());
    tracing::info!(version = HOST_VERSION, protocol_version = PROTOCOL_VERSION, config = %config_path.display(), port = config.port, "DeckyPowerHost starting in safe development mode");
    let listener = server::bind(config.port)
        .await
        .map_err(|error| anyhow::anyhow!("could not listen on 0.0.0.0:{}: {error}", config.port))?;
    let hostname = hostname::get()?.to_string_lossy().into_owned();
    server::serve(
        listener,
        Arc::new(AppState {
            identity: Mutex::new(identity),
            pairing: Mutex::new(pairing),
            authenticator: Authenticator::default(),
            power: Arc::new(MockPowerController::default()),
            store,
            hostname,
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
    if !arguments.iter().any(|argument| argument == "--dev")
        && !arguments
            .iter()
            .any(|argument| argument == "--pairing-code" || argument == "--reset-pairing")
    {
        let directory = std::env::var_os("ProgramData")
            .map(PathBuf::from)
            .ok_or_else(|| anyhow::anyhow!("ProgramData is unavailable"))?
            .join("DeckyPowerHost");
        std::fs::create_dir_all(&directory)?;
        let appender = tracing_appender::rolling::never(directory, "DeckyPowerHost.log");
        let (writer, guard) = tracing_appender::non_blocking(appender);
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_ansi(false)
            .with_writer(writer)
            .init();
        return Ok(Some(guard));
    }
    let _ = arguments;
    tracing_subscriber::fmt().with_env_filter(filter).init();
    Ok(None)
}

fn config_override(arguments: &[String]) -> Option<PathBuf> {
    arguments
        .windows(2)
        .find(|pair| pair[0] == "--config")
        .map(|pair| PathBuf::from(&pair[1]))
}
