use std::{
    collections::HashMap,
    env, fs,
    io::Write,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use chacha20poly1305::{
    ChaCha20Poly1305, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use decky_power_host::{
    PROTOCOL_VERSION,
    auth::{now_unix, random_nonce, sign, verify_response},
    pairing::{CredentialContext, credential_aad},
    protocol::{
        ErrorResponse, PairRequest, PairResponse, ShutdownRequest, ShutdownResponse, StatusRequest,
        StatusResponse,
    },
};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use prost::Message;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use spake2::{Ed25519Group, Identity, Password, Spake2};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

const CONTENT_TYPE: &str = "application/x-protobuf";
const CONFIRMATION_DOMAIN: &[u8] = b"deckypower-pairing-confirm-v1\0";
const CREDENTIAL_INFO: &[u8] = b"deckypower-pairing-credential-v1";

#[derive(Serialize, Deserialize)]
struct SavedCredential {
    host_id: String,
    credential: String,
}

struct Options {
    command: String,
    host: String,
    port: u16,
    code: Option<String>,
    credential_file: PathBuf,
}

struct RequestAuthentication {
    headers: Vec<(String, String)>,
    credential: Vec<u8>,
    nonce: [u8; 16],
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("decky-power-test: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let options = parse_options()?;
    match options.command.as_str() {
        "pair" => pair(&options).await,
        "status" => status(&options).await,
        "shutdown" => shutdown(&options).await,
        _ => Err(usage()),
    }
}

fn parse_options() -> Result<Options, String> {
    parse_arguments(env::args().skip(1))
}

fn parse_arguments(mut arguments: impl Iterator<Item = String>) -> Result<Options, String> {
    let command = arguments.next().ok_or_else(usage)?;
    let mut values = HashMap::new();
    while let Some(name) = arguments.next() {
        let value = arguments
            .next()
            .ok_or_else(|| format!("missing value for {name}\n{}", usage()))?;
        values.insert(name, value);
    }
    let host = values
        .remove("--host")
        .ok_or_else(|| format!("--host is required\n{}", usage()))?;
    let port = values
        .remove("--port")
        .unwrap_or_else(|| "47991".into())
        .parse::<u16>()
        .map_err(|_| "--port must be in 1..=65535".to_owned())?;
    if port == 0 {
        return Err("--port must be in 1..=65535".into());
    }
    let credential_file = values
        .remove("--credential-file")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".decky-power-test.json"));
    let code = values.remove("--code");
    if !values.is_empty() {
        return Err(format!(
            "unknown arguments: {:?}\n{}",
            values.keys(),
            usage()
        ));
    }
    Ok(Options {
        command,
        host,
        port,
        code,
        credential_file,
    })
}

fn usage() -> String {
    "usage: decky-power-test <pair|status|shutdown> --host HOST [--port PORT] [--code 483921] [--credential-file PATH]".into()
}

async fn pair(options: &Options) -> Result<(), String> {
    let code = options
        .code
        .as_deref()
        .ok_or_else(|| "pair requires --code".to_owned())?
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    if code.len() != 6 || !code.bytes().all(|value| value.is_ascii_digit()) {
        return Err("pairing code must contain six digits".into());
    }
    let (client, client_message) = Spake2::<Ed25519Group>::start_a(
        &Password::new(code.as_bytes()),
        &Identity::new(b"decky-client"),
        &Identity::new(b"decky-host"),
    );
    let request = PairRequest {
        protocol_version: PROTOCOL_VERSION,
        client_version: env!("CARGO_PKG_VERSION").into(),
        client_spake2_message: client_message.clone(),
        session_id: Vec::new(),
        client_confirmation: Vec::new(),
    }
    .encode_to_vec();
    let started = PairResponse::decode(
        post(options, "/v1/pair", &request, &[], None)
            .await?
            .as_slice(),
    )
    .map_err(|error| format!("invalid pairing response: {error}"))?;
    let shared = client
        .finish(&started.host_spake2_message)
        .map_err(|_| "pairing exchange failed".to_owned())?;
    let mut confirmation = <Hmac<Sha256> as Mac>::new_from_slice(&shared)
        .map_err(|_| "pairing confirmation failed".to_owned())?;
    confirmation.update(CONFIRMATION_DOMAIN);
    confirmation.update(&client_message);
    confirmation.update(&started.host_spake2_message);
    confirmation.update(&started.session_id);
    let request = PairRequest {
        protocol_version: PROTOCOL_VERSION,
        client_version: env!("CARGO_PKG_VERSION").into(),
        client_spake2_message: Vec::new(),
        session_id: started.session_id,
        client_confirmation: confirmation.finalize().into_bytes().to_vec(),
    }
    .encode_to_vec();
    let paired = PairResponse::decode(
        post(options, "/v1/pair", &request, &[], None)
            .await?
            .as_slice(),
    )
    .map_err(|error| format!("invalid pairing response: {error}"))?;
    let mut key = [0_u8; 32];
    Hkdf::<Sha256>::new(None, &shared)
        .expand(CREDENTIAL_INFO, &mut key)
        .map_err(|_| "credential derivation failed".to_owned())?;
    let credential = ChaCha20Poly1305::new((&key).into())
        .decrypt(
            Nonce::from_slice(&paired.encryption_nonce),
            Payload {
                msg: &paired.encrypted_credential,
                aad: &credential_aad(
                    &paired.host_spake2_message,
                    &paired.session_id,
                    &CredentialContext {
                        hostname: &paired.hostname,
                        host_version: &paired.host_version,
                        protocol_version: paired.protocol_version,
                        host_id: &paired.host_id,
                    },
                )
                .map_err(|_| "host returned invalid pairing metadata".to_owned())?,
            },
        )
        .map_err(|_| "the pairing code was rejected".to_owned())?;
    if credential.len() != 32 {
        return Err("host returned an invalid credential".into());
    }
    let saved = SavedCredential {
        host_id: paired.host_id.clone(),
        credential: hex::encode(credential),
    };
    write_credential_file(
        &options.credential_file,
        &serde_json::to_vec_pretty(&saved).map_err(|error| error.to_string())?,
    )
    .map_err(|error| {
        format!(
            "could not save {}: {error}",
            options.credential_file.display()
        )
    })?;
    println!("paired with {} ({})", paired.hostname, paired.host_id);
    Ok(())
}

fn write_credential_file(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    #[cfg(unix)]
    file.set_permissions({
        use std::os::unix::fs::PermissionsExt;
        fs::Permissions::from_mode(0o600)
    })?;
    file.write_all(contents)?;
    file.sync_all()
}

async fn status(options: &Options) -> Result<(), String> {
    let saved = load_credential(options)?;
    let body = StatusRequest {
        protocol_version: PROTOCOL_VERSION,
        client_version: env!("CARGO_PKG_VERSION").into(),
    }
    .encode_to_vec();
    let authentication = authentication_headers("/v1/status", &body, &saved.credential)?;
    let response = StatusResponse::decode(
        post(
            options,
            "/v1/status",
            &body,
            &authentication.headers,
            Some((&authentication.credential, &authentication.nonce)),
        )
        .await?
        .as_slice(),
    )
    .map_err(|error| format!("invalid status response: {error}"))?;
    if response.host_id != saved.host_id {
        return Err("host identity differs from the paired host".into());
    }
    println!(
        "online: {} host={} protocol={}",
        response.hostname, response.host_version, response.protocol_version
    );
    Ok(())
}

async fn shutdown(options: &Options) -> Result<(), String> {
    let saved = load_credential(options)?;
    let body = ShutdownRequest {
        protocol_version: PROTOCOL_VERSION,
        client_version: env!("CARGO_PKG_VERSION").into(),
    }
    .encode_to_vec();
    let authentication = authentication_headers("/v1/shutdown", &body, &saved.credential)?;
    let response = ShutdownResponse::decode(
        post(
            options,
            "/v1/shutdown",
            &body,
            &authentication.headers,
            Some((&authentication.credential, &authentication.nonce)),
        )
        .await?
        .as_slice(),
    )
    .map_err(|error| format!("invalid shutdown response: {error}"))?;
    if !response.accepted {
        return Err("host rejected shutdown".into());
    }
    println!("shutdown accepted (the host controls whether shutdown is mocked)");
    Ok(())
}

fn load_credential(options: &Options) -> Result<SavedCredential, String> {
    let bytes = fs::read(&options.credential_file).map_err(|error| {
        format!(
            "could not read {}: {error}",
            options.credential_file.display()
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|error| format!("invalid credential file: {error}"))
}

fn authentication_headers(
    path: &str,
    body: &[u8],
    credential_hex: &str,
) -> Result<RequestAuthentication, String> {
    let credential =
        hex::decode(credential_hex).map_err(|_| "invalid stored credential".to_owned())?;
    let timestamp = now_unix();
    let nonce = random_nonce();
    let signature = sign(&credential, timestamp, &nonce, "POST", path, body)
        .map_err(|error| error.to_string())?;
    Ok(RequestAuthentication {
        headers: vec![
            ("x-decky-timestamp".into(), timestamp.to_string()),
            ("x-decky-nonce".into(), hex::encode(nonce)),
            ("x-decky-signature".into(), hex::encode(signature)),
        ],
        credential,
        nonce,
    })
}

async fn post(
    options: &Options,
    path: &str,
    body: &[u8],
    headers: &[(String, String)],
    response_authentication: Option<(&[u8], &[u8; 16])>,
) -> Result<Vec<u8>, String> {
    let address = format!("{}:{}", options.host, options.port);
    let mut stream = TcpStream::connect(&address)
        .await
        .map_err(|error| format!("could not connect to {address}: {error}"))?;
    let peer: SocketAddr = stream.peer_addr().map_err(|error| error.to_string())?;
    let mut request = format!(
        "POST {path} HTTP/1.1\r\nHost: {peer}\r\nContent-Type: {CONTENT_TYPE}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (name, value) in headers {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    request.push_str("\r\n");
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|error| error.to_string())?;
    stream
        .write_all(body)
        .await
        .map_err(|error| error.to_string())?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .await
        .map_err(|error| error.to_string())?;
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| "malformed HTTP response".to_owned())?;
    let response_headers = std::str::from_utf8(&response[..header_end])
        .map_err(|_| "malformed HTTP headers".to_owned())?;
    let status = response_headers
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| "malformed HTTP status".to_owned())?;
    let response_body = response[header_end + 4..].to_vec();
    if let Some((credential, request_nonce)) = response_authentication {
        let signature = response_headers
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(name, _)| name.eq_ignore_ascii_case("x-decky-response-signature"))
            .map(|(_, value)| value.trim())
            .ok_or_else(|| "host response authentication is missing".to_owned())?;
        let signature = hex::decode(signature)
            .map_err(|_| "host response authentication is malformed".to_owned())?;
        verify_response(
            credential,
            request_nonce,
            path,
            status,
            &response_body,
            &signature,
        )
        .map_err(|_| "host response authentication failed".to_owned())?;
    }
    if !(200..300).contains(&status) {
        let detail = ErrorResponse::decode(response_body.as_slice())
            .map(|error| error.message)
            .unwrap_or_else(|_| "host returned a malformed error".into());
        return Err(format!("host returned HTTP {status}: {detail}"));
    }
    Ok(response_body)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(values: &[&str]) -> Result<Options, String> {
        parse_arguments(values.iter().map(|value| (*value).to_owned()))
    }

    #[test]
    fn port_default_custom_and_boundaries_are_validated() {
        assert_eq!(parse(&["status", "--host", "pc"]).unwrap().port, 47_991);
        assert_eq!(
            parse(&["status", "--host", "pc", "--port", "48100"])
                .unwrap()
                .port,
            48_100
        );
        assert_eq!(
            parse(&["status", "--host", "pc", "--port", "1"])
                .unwrap()
                .port,
            1
        );
        assert_eq!(
            parse(&["status", "--host", "pc", "--port", "65535"])
                .unwrap()
                .port,
            65_535
        );
        for value in ["0", "65536", "not-a-port"] {
            assert!(parse(&["status", "--host", "pc", "--port", value]).is_err());
        }
    }

    #[test]
    fn misleading_mock_client_flag_is_rejected() {
        assert!(parse(&["shutdown", "--host", "pc", "--mock"]).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn credential_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("credential.json");
        write_credential_file(&path, b"secret").unwrap();
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
