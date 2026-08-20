# Architecture

Decky Remote PC Power is split into three trust boundaries: an unprivileged React
view, the Decky Python backend, and the native Windows `DeckyPowerHost` service.
Only the backend can access credentials or the network protocol.

## Repository layout

- `decky/src/`: strict TypeScript Quick Access and settings UI.
- `decky/main.py` and `decky/py_modules/decky_power/`: Decky backend.
- `proto/`: the versioned wire contract shared by both implementations.
- `host/`: portable Rust server core and narrowly gated Windows integration.
- `host/installer/`: Inno Setup installer owned by the Windows host.
- `docs/`: protocol, security, and validation documentation.

## Device and persistence model

Settings schema version 1 stores independently addressable devices. Each device
has a UUID, display name, address, normalized MAC, persisted manual-override
choice, port, optional broadcast
address, paired host ID, host metadata, and a reference to backend-only secret
material. The address is never identity. A successful authenticated response
whose host ID differs from the paired host is rejected.

The Decky backend stores settings in `DECKY_PLUGIN_SETTINGS_DIR`. Its secret file
is mode `0600` and never returned by configuration APIs. SteamOS does not provide
a dependable system keyring to all Decky plugins, so this is access control, not
encryption at rest; a local process with the `deck` user's privileges can recover
the credentials. Windows credentials are stored under ProgramData and encrypted
with machine-scope DPAPI so the LocalSystem service can recover them after boot.

## State machine

Each PC moves independently through `offline`, `starting`, `online`, `stopping`,
and `unknown`. Authenticated status is authoritative. Wake sends UDP magic
packets and enters `starting`; shutdown is accepted before entering `stopping`.
Transient states poll every two seconds and expire after 120 seconds. Steady
states poll every 15 seconds while the panel is mounted. Per-device locks prevent
duplicate actions, and concurrent status requests keep one slow host from
blocking another.

## Host protocol

HTTP/1.1 carries `application/x-protobuf` messages:

| Method | Path | Authentication | Purpose |
| --- | --- | --- | --- |
| POST | `/v1/status` | HMAC after pairing | Availability and version |
| POST | `/v1/pair` | SPAKE2 pairing code | Establish credential |
| POST | `/v1/shutdown` | HMAC required | Local shutdown only |

There is no generic execution or file API.

### Pairing

The host generates a six-digit code from the operating-system CSPRNG, valid for
five minutes and five attempts. The Deck and host run SPAKE2 with fixed role
identities (`decky-client` and `decky-host`) and the code as the password. The
client sends its SPAKE2 message and the host returns its message plus a random
session ID. The client sends an HMAC key confirmation covering both messages and
the session ID. Only after confirmation succeeds does the host create a 32-byte
random credential and return it encrypted with ChaCha20-Poly1305 under an
HKDF-SHA256 key derived from the SPAKE2 shared key. This second round prevents a
wrong code from leaving the host paired to an unusable credential. The successful
code is invalidated. The host then stores the credential and stable host UUID.

SPAKE2 prevents a passive LAN observer from using the transcript for an offline
guess of the short code. Online attempts remain rate-limited. Pairing does not
hide host metadata or traffic timing and assumes the code display itself is
trusted. A malicious endpoint that can replace the host executable or read Deck
plugin settings is outside the home-LAN attacker model.

### Request authentication

Authenticated requests carry hexadecimal headers `X-Decky-Timestamp`,
`X-Decky-Nonce`, and `X-Decky-Signature`. The signed byte sequence is:

```
"deckypower-auth-v1\0" ||
u64_be(timestamp) ||
u16_be(nonce_length) || nonce ||
u16_be(method_length) || uppercase_ascii_method ||
u16_be(path_length) || ascii_path ||
sha256(body)
```

The signature is HMAC-SHA256 with the paired credential. The host accepts a
60-second clock window, compares tags in constant time, and retains accepted
nonces for the clock window. A nonce is 16 random bytes. Authentication failures
are globally bounded to 20 per clock window and no secret-bearing headers are logged.

### TLS decision

TLS would encrypt metadata but creates certificate bootstrapping and pinning
complexity without replacing pairing. The PAKE already establishes a secret
without exposing it, and HMAC authenticates every state-changing request, so v1
uses HTTP on a Private-profile LAN firewall. This does not provide confidentiality
for status metadata and cannot conceal traffic from a local observer. A future
additive protocol can negotiate TLS using the paired host identity if that threat
model becomes a requirement.

## Windows host

Portable server, pairing, configuration, and authentication code is isolated
from `service/windows.rs`, `power/windows.rs`, and `storage/windows.rs`. The SCM
entry reports service state and uses a stop event to gracefully release the
listener. `WindowsPowerController` enables `SeShutdownPrivilege` and calls the
Unicode `InitiateShutdownW` API with a planned application-maintenance reason.
Tests always inject `MockPowerController`.

The service binds `0.0.0.0` at the validated TOML port. Network restriction is
provided by the installer firewall rule limited to the Windows Private profile.
Changing the port requires running the installer-provided firewall sync command,
restarting the service, and updating the matching Decky device.

## Installer

Inno Setup was selected for a small native bootstrapper, stable upgrades via a
fixed AppId, elevation, and concise service/firewall lifecycle scripting. The
config entry uses `onlyifdoesntexist`, preserving custom ports on upgrade.
Credential state and TOML are retained on uninstall for reinstall continuity;
the documentation gives the exact paths for permanent manual removal. The
firewall rule and service are always removed.
