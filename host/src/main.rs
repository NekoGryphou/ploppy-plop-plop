#[cfg(windows)]
use rand::Rng;
use std::{path::PathBuf, sync::Arc};

#[cfg(windows)]
use std::{fs::OpenOptions, io};

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
    if is_interactive_pairing(&arguments) {
        return show_pairing_code(&arguments);
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
    if arguments.iter().any(|arg| arg == "--service") {
        #[cfg(windows)]
        return decky_power_host::service::windows::dispatch();
        #[cfg(not(windows))]
        anyhow::bail!("service mode is only supported on Windows");
    }
    if !arguments.iter().any(|arg| arg == "--dev") {
        anyhow::bail!(
            "unknown arguments; use --dev --mock-shutdown for development or --pairing-code"
        );
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
    if arguments.iter().any(|argument| argument == "--service") {
        let directory = std::env::var_os("ProgramData")
            .map(PathBuf::from)
            .ok_or_else(|| anyhow::anyhow!("ProgramData is unavailable"))?
            .join("DeckyPowerHost");
        let log_path = directory.join("DeckyPowerHost.log");
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

#[cfg(windows)]
fn is_interactive_pairing(arguments: &[String]) -> bool {
    arguments.len() == 1 || arguments.iter().any(|arg| arg == "--pairing-code")
}

#[cfg(windows)]
fn show_pairing_code(arguments: &[String]) -> anyhow::Result<()> {
    use decky_power_host::storage::windows::WindowsCredentialStore;

    let store = WindowsCredentialStore::program_data()?;
    let mut identity = match store.load_or_create() {
        Ok(identity) => identity,
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            if arguments.iter().any(|arg| arg == "--elevated-pairing") {
                anyhow::bail!(
                    "access to protected pairing state was denied even after elevation: {error}"
                );
            }
            elevate_pairing_helper()?;
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    let message = if identity.credential.is_some() {
        "DeckyPowerHost is already paired. To replace the paired Steam Deck, run DeckyPowerHost.exe --reset-pairing from an Administrator terminal.".to_owned()
    } else {
        identity.pairing_code = Some(format!("{:06}", rand::rng().random_range(0..1_000_000)));
        identity.pairing_created_at = decky_power_host::auth::now_unix();
        store.save(&identity)?;
        format!(
            "DeckyPowerHost pairing code: {}\n\nThis code expires after five minutes.",
            identity.pairing_code.as_deref().unwrap_or("unavailable")
        )
    };
    println!("{message}");
    if arguments.iter().any(|arg| arg == "--show-dialog") {
        show_message(&message);
    }
    Ok(())
}

#[cfg(windows)]
fn elevate_pairing_helper() -> anyhow::Result<()> {
    use windows::{
        Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
        core::PCWSTR,
    };

    let executable = std::env::current_exe()?;
    let executable_wide = wide(executable.as_os_str());
    let operation = wide(std::ffi::OsStr::new("runas"));
    let parameters = wide(std::ffi::OsStr::new(
        "--pairing-code --elevated-pairing --show-dialog",
    ));
    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(operation.as_ptr()),
            PCWSTR(executable_wide.as_ptr()),
            PCWSTR(parameters.as_ptr()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize <= 32 {
        anyhow::bail!(
            "administrator permission is required to access protected pairing state (ShellExecute error {})",
            result.0 as isize
        );
    }
    Ok(())
}

#[cfg(windows)]
fn show_message(message: &str) {
    use windows::{
        Win32::UI::WindowsAndMessaging::{MB_ICONINFORMATION, MB_OK, MessageBoxW},
        core::PCWSTR,
    };

    let title = wide(std::ffi::OsStr::new("DeckyPowerHost"));
    let message = wide(std::ffi::OsStr::new(message));
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(message.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

#[cfg(windows)]
fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    value.encode_wide().chain(std::iter::once(0)).collect()
}

fn config_override(arguments: &[String]) -> Option<PathBuf> {
    arguments
        .windows(2)
        .find(|pair| pair[0] == "--config")
        .map(|pair| PathBuf::from(&pair[1]))
}
