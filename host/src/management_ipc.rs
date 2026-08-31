use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::windows::named_pipe::NamedPipeServer,
};
use windows::{
    Win32::{
        Foundation::{HLOCAL, LocalFree},
        Security::{
            Authorization::{
                ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
            },
            PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
        },
        Storage::FileSystem::{
            FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX,
        },
        System::Pipes::{
            CreateNamedPipeW, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES,
            PIPE_WAIT,
        },
    },
    core::w,
};

use crate::{management, server::AppState};

pub const PIPE_NAME: &str = r"\\.\pipe\DeckyPowerHostControl";
const PIPE_SDDL: windows::core::PCWSTR = w!("D:P(A;;GA;;;SY)(A;;GA;;;BA)");

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Operation {
    GetServiceInfo,
    GetPairingState,
    GeneratePairingCode,
}

#[derive(Deserialize)]
struct ManagementRequest {
    operation: Operation,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ManagementResponse {
    ok: bool,
    error: Option<String>,
    service_running: bool,
    port: u16,
    paired: bool,
    pairing_code: Option<String>,
    expires_in_seconds: u64,
    host_version: &'static str,
    plugin_version: Option<String>,
    version_status: &'static str,
}

pub async fn serve(state: Arc<AppState>, port: u16) -> std::io::Result<()> {
    let mut first = true;
    let mut consecutive_failures = 0_u32;
    loop {
        let server = match create_pipe(first) {
            Ok(server) => server,
            Err(error) => {
                consecutive_failures += 1;
                if consecutive_failures >= 5 {
                    return Err(error);
                }
                tracing::warn!(%error, attempt = consecutive_failures, "could not create local management pipe; retrying");
                tokio::time::sleep(retry_delay(consecutive_failures)).await;
                continue;
            }
        };
        first = false;
        if let Err(error) = server.connect().await {
            consecutive_failures += 1;
            if consecutive_failures >= 5 {
                return Err(error);
            }
            tracing::warn!(%error, attempt = consecutive_failures, "local management pipe connection failed; retrying");
            tokio::time::sleep(retry_delay(consecutive_failures)).await;
            continue;
        }
        consecutive_failures = 0;
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(error) = handle(server, &state, port).await {
                tracing::warn!(%error, "local management pipe request failed");
            }
        });
    }
}

fn retry_delay(attempt: u32) -> std::time::Duration {
    std::time::Duration::from_millis(100_u64.saturating_mul(1_u64 << attempt.min(4)))
}

fn create_pipe(first: bool) -> std::io::Result<NamedPipeServer> {
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PIPE_SDDL,
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )
        .map_err(std::io::Error::other)?;
    }
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: false.into(),
    };
    let mut open_mode = PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED;
    if first {
        open_mode |= FILE_FLAG_FIRST_PIPE_INSTANCE;
    }
    let handle = unsafe {
        CreateNamedPipeW(
            w!(r"\\.\pipe\DeckyPowerHostControl"),
            open_mode,
            PIPE_TYPE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            PIPE_UNLIMITED_INSTANCES,
            4096,
            4096,
            0,
            Some(&attributes),
        )
    };
    unsafe { LocalFree(Some(HLOCAL(descriptor.0))) };
    if handle.is_invalid() {
        return Err(std::io::Error::last_os_error());
    }
    use std::os::windows::io::RawHandle;
    unsafe { NamedPipeServer::from_raw_handle(handle.0 as RawHandle) }
}

async fn handle(
    mut pipe: tokio::net::windows::named_pipe::NamedPipeServer,
    state: &AppState,
    port: u16,
) -> std::io::Result<()> {
    let mut length_bytes = [0_u8; 4];
    pipe.read_exact(&mut length_bytes).await?;
    let request_length = u32::from_le_bytes(length_bytes) as usize;
    if request_length == 0 || request_length > 4096 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid management request length",
        ));
    }
    let mut request_bytes = vec![0_u8; request_length];
    pipe.read_exact(&mut request_bytes).await?;
    let request: ManagementRequest = serde_json::from_slice(&request_bytes)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    let pairing = match request.operation {
        Operation::GetServiceInfo | Operation::GetPairingState => {
            Ok(management::pairing_state(state).await)
        }
        Operation::GeneratePairingCode => management::generate_pairing_code(state).await,
    };
    let response = match pairing {
        Ok(pairing) => {
            let plugin_version = state.latest_client_version.lock().await.clone();
            let version_status = match plugin_version.as_deref() {
                Some(version) => match crate::versioning::compare(version, crate::HOST_VERSION) {
                    crate::versioning::VersionRelation::Compatible => "compatible",
                    crate::versioning::VersionRelation::UpdateHost => "update_host",
                    crate::versioning::VersionRelation::UpdatePlugin => "update_plugin",
                    crate::versioning::VersionRelation::Incompatible => "incompatible",
                    crate::versioning::VersionRelation::Unknown => "unknown",
                },
                None => "unknown",
            };
            ManagementResponse {
                ok: true,
                error: None,
                service_running: true,
                port,
                paired: pairing.paired,
                pairing_code: pairing.code,
                expires_in_seconds: pairing.expires_in.as_secs(),
                host_version: crate::HOST_VERSION,
                plugin_version,
                version_status,
            }
        }
        Err(error) => ManagementResponse {
            ok: false,
            error: Some(error.to_string()),
            service_running: true,
            port,
            paired: false,
            pairing_code: None,
            expires_in_seconds: 0,
            host_version: crate::HOST_VERSION,
            plugin_version: None,
            version_status: "unknown",
        },
    };
    let response_bytes = serde_json::to_vec(&response).map_err(std::io::Error::other)?;
    pipe.write_all(&(response_bytes.len() as u32).to_le_bytes())
        .await?;
    pipe.write_all(&response_bytes).await?;
    pipe.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{management::state_for_test, storage::DevelopmentStore};
    use tokio::net::windows::named_pipe::ClientOptions;

    #[test]
    fn pipe_acl_is_limited_to_system_and_administrators() {
        let sddl = unsafe { PIPE_SDDL.to_string() }.unwrap();
        assert_eq!(sddl, "D:P(A;;GA;;;SY)(A;;GA;;;BA)");
        assert!(!sddl.contains(";;;IU)"));
        assert!(!sddl.contains(";;;AU)"));
        assert!(!sddl.contains(";;;WD)"));
    }

    #[test]
    fn management_retries_use_bounded_backoff() {
        assert_eq!(retry_delay(1), std::time::Duration::from_millis(200));
        assert_eq!(retry_delay(4), std::time::Duration::from_millis(1_600));
        assert_eq!(retry_delay(20), std::time::Duration::from_millis(1_600));
    }

    #[tokio::test]
    async fn named_pipe_generates_code_through_framed_local_protocol() {
        let directory = tempfile::tempdir().unwrap();
        let state = state_for_test(
            Arc::new(DevelopmentStore {
                path: directory.path().join("identity.json"),
            }),
            "ipc-test",
        )
        .await
        .unwrap();
        *state.latest_client_version.lock().await = Some("0.2.0".into());
        let task = tokio::spawn(serve(state, 48100));
        let mut client = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                match ClientOptions::new().open(PIPE_NAME) {
                    Ok(client) => break client,
                    Err(error) if matches!(error.raw_os_error(), Some(2) | Some(231)) => {
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await
                    }
                    Err(error) => panic!("management pipe open failed: {error}"),
                }
            }
        })
        .await
        .expect("management pipe did not become available");
        let request = br#"{"operation":"generate_pairing_code"}"#;
        client
            .write_all(&(request.len() as u32).to_le_bytes())
            .await
            .unwrap();
        client.write_all(request).await.unwrap();
        client.flush().await.unwrap();
        let mut length = [0_u8; 4];
        client.read_exact(&mut length).await.unwrap();
        let mut response = vec![0_u8; u32::from_le_bytes(length) as usize];
        client.read_exact(&mut response).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&response).unwrap();
        assert_eq!(value["ok"], true);
        assert_eq!(value["port"], 48100);
        assert_eq!(value["hostVersion"], crate::HOST_VERSION);
        assert_eq!(value["pluginVersion"], "0.2.0");
        assert_eq!(value["versionStatus"], "update_host");
        assert_eq!(value["pairingCode"].as_str().unwrap().len(), 6);
        task.abort();
    }
}
