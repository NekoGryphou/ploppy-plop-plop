use std::{
    ffi::OsString,
    sync::{Arc, mpsc},
    time::Duration,
};

use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};

use crate::{
    HOST_VERSION, PROTOCOL_VERSION,
    auth::Authenticator,
    config::HostConfig,
    pairing::PairingCode,
    power::windows::WindowsPowerController,
    server::{self, AppState},
    storage::{CredentialStore, windows::WindowsCredentialStore},
};

const SERVICE_NAME: &str = "DeckyMyRigHost";

define_windows_service!(ffi_service_main, service_main);

pub fn dispatch() -> anyhow::Result<()> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
    Ok(())
}

fn service_main(_arguments: Vec<OsString>) {
    if let Err(error_value) = run_service() {
        tracing::error!(error = %error_value, "DeckyMyRigHost service stopped with an error");
    }
}

fn run_service() -> anyhow::Result<()> {
    let (stop_sender, stop_receiver) = mpsc::channel();
    let status_handle =
        service_control_handler::register(SERVICE_NAME, move |control| match control {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                let _ = stop_sender.send(());
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        })?;
    status_handle.set_service_status(status(
        ServiceState::StartPending,
        ServiceControlAccept::empty(),
        15_000,
    ))?;
    let config_path = HostConfig::next_to_executable()?;
    let config = HostConfig::load(&config_path)?;
    let store: Arc<dyn CredentialStore> = Arc::new(WindowsCredentialStore::program_data()?);
    let identity = store.load_or_create()?;
    let latest_client_version = identity.last_client_version.clone();
    let pairing = if let Some(code) = identity.pairing_code.clone() {
        PairingCode::from_code_with_age(
            code,
            crate::management::persisted_code_age(
                identity.pairing_created_at,
                crate::auth::now_unix(),
            ),
        )?
    } else {
        PairingCode::generate()
    };
    tracing::info!(version = HOST_VERSION, protocol_version = PROTOCOL_VERSION, config = %config_path.display(), port = config.port, "DeckyMyRigHost service starting");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let result = runtime.block_on(async move {
        let listener = server::bind(config.port).await.map_err(|error| {
            anyhow::anyhow!("could not listen on 0.0.0.0:{}: {error}", config.port)
        })?;
        let hostname = hostname::get()?.to_string_lossy().into_owned();
        let state = Arc::new(AppState {
            identity: tokio::sync::Mutex::new(identity),
            pairing: tokio::sync::Mutex::new(pairing),
            authenticator: Authenticator::default(),
            power: Arc::new(WindowsPowerController),
            store,
            hostname,
            latest_client_version: tokio::sync::Mutex::new(latest_client_version),
        });
        let mut management = tokio::spawn(crate::management_ipc::serve(state.clone(), config.port));
        status_handle.set_service_status(status(
            ServiceState::Running,
            ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
            0,
        ))?;
        let http = async move {
            axum::serve(
                listener,
                server::router(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
                .with_graceful_shutdown(async move {
                    let _ = tokio::task::spawn_blocking(move || stop_receiver.recv()).await;
                })
                .await
        };
        tokio::pin!(http);
        tokio::select! {
            server_result = &mut http => {
                management.abort();
                server_result?;
            }
            management_result = &mut management => {
                match management_result {
                    Ok(Ok(())) => anyhow::bail!("local management pipe stopped unexpectedly"),
                    Ok(Err(error)) => return Err(anyhow::anyhow!("local management pipe failed: {error}")),
                    Err(error) => return Err(anyhow::anyhow!("local management pipe task failed: {error}")),
                }
            }
        }
        Ok::<(), anyhow::Error>(())
    });
    status_handle.set_service_status(status(
        ServiceState::Stopped,
        ServiceControlAccept::empty(),
        0,
    ))?;
    result
}

fn status(state: ServiceState, accepted: ServiceControlAccept, wait_hint_ms: u64) -> ServiceStatus {
    ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: state,
        controls_accepted: accepted,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::from_millis(wait_hint_ms),
        process_id: None,
    }
}
