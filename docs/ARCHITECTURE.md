# Architecture

Decky My Rig is split into four trust boundaries: an unprivileged React
view, the Decky Python backend, the native Windows `DeckyMyRigHost` service, and
the interactive WinUI 3 control application. Only the Decky backend can access
Deck-side credentials or the LAN protocol. The WinUI application uses a narrow
local named pipe and never owns pairing state.

## Repository layout

- `decky/src/`: strict TypeScript Quick Access and settings UI.
- `decky/main.py` and `decky/py_modules/decky_my_rig/`: Decky backend.
- `proto/`: the versioned wire contract shared by both implementations.
- `host/`: complete Windows host product: portable Rust server core, narrowly
  gated Windows integration, WinUI 3 control application, and installer.
- `host/control/`: WinUI 3 application plus testable view-model/client logic.
- `host/installer/`: Inno Setup installer owned by the Windows host.
- `docs/`: protocol, security, and validation documentation.

## Device and persistence model

Settings schema version 2 stores independently addressable devices. Each device
has a UUID, display name, address, normalized MAC, persisted manual-override
choice, port, optional broadcast
address, paired host ID, host metadata, and a reference to backend-only secret
material. Pairing credentials are optional and stored separately; a device is a
valid persisted configuration without one. The address is never identity. A successful authenticated response
whose host ID differs from the paired host is rejected.

The Decky backend stores settings in `DECKY_PLUGIN_SETTINGS_DIR`. Its secret file
is mode `0600` and never returned by configuration APIs. SteamOS does not provide
a dependable system keyring to all Decky plugins, so this is access control, not
encryption at rest; a local process with the `deck` user's privileges can recover
the credentials. Changes spanning settings and credentials use a mode-`0600`
write-ahead transaction document. Normal write failures roll back, while a crash
or power loss is detected and rolled forward before the next read, preventing
orphaned credentials and half-applied pairing state. Windows credentials are
stored under ProgramData and encrypted
with machine-scope DPAPI so the LocalSystem service can recover them after boot.

## State machine

Power/reachability and pairing are separate dimensions. Each PC moves
independently through `offline`, `starting`, `online`, `stopping`, and `unknown`,
while pairing remains unpaired, paired, pairing, or failed/expired. Combined
states such as Offline + Unpaired and Online + Paired are valid. A reachable host
with no Deck credential is pairing-required rather than offline. Authenticated
status is authoritative. Wake sends UDP magic packets and enters `starting`;
shutdown is accepted before entering `stopping`.
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

Authenticated status and shutdown requests include the plugin's strict semantic
version. The host records only versions from successfully authenticated requests
and exposes the most recently observed value to the administrator-only management
pipe. Version comparison intentionally ignores patch differences: a minor
difference directs the user to update the older component, while a major
difference is reported as incompatible. Missing or malformed versions are
reported as unknown. This negotiation is independent from protocol-version
enforcement and from the persisted pairing identity, so upgrading either package
does not require pairing again.

### Pairing

The host generates a six-digit code from the operating-system CSPRNG, valid for
five minutes and five failed key confirmations per source. Pending starts are capped per
source address and globally. The Deck and host run SPAKE2 with fixed role
identities (`decky-client` and `decky-host`) and the code as the password. The
client sends its SPAKE2 message and the host returns its message plus a random
session ID. The client sends an HMAC key confirmation covering both messages and
the session ID. Only after confirmation succeeds does the host create a 32-byte
random credential and return it encrypted with ChaCha20-Poly1305 under an
HKDF-SHA256 key derived from the SPAKE2 shared key. The AEAD associated data
binds the host SPAKE2 message, session ID, hostname, host version, protocol
version, and stable host ID so none of the returned identity metadata can be
substituted. This second round prevents a
wrong code from leaving the host paired to an unusable credential. On success,
the host first stores the credential and stable host UUID, then invalidates the code.
The encrypted completion is retained for a bounded recovery window, so an
identical final request can recover from a lost response without rotating the
credential again.

The service is the sole source of truth for pairing-code generation, expiration,
attempt limiting, and regeneration. `DeckyMyRigHostControl` requests only
service info, pairing state, or a new code over
`\\.\pipe\DeckyMyRigHostControl`. The pipe rejects remote clients and its ACL
grants access only to LocalSystem and local Administrators. The WinUI application
requests elevation before connecting. No LAN route returns the current code.
Transient pipe creation and connection failures use bounded exponential retry;
repeated failure terminates the supervised service instead of leaving a false
Running state without management access.
Generating a replacement code invalidates the previous code; an existing
credential remains usable until re-pairing succeeds.

SPAKE2 prevents a passive LAN observer from using the transcript for an offline
guess of the short code. Online attempts remain rate-limited. Pairing does not
hide host metadata or traffic timing and assumes the code display itself is
trusted. A malicious endpoint that can replace the host executable or read Deck
plugin settings is outside the home-LAN attacker model.

### Request authentication

Authenticated requests carry hexadecimal headers `X-Decky-Timestamp`,
`X-Decky-Nonce`, and `X-Decky-Signature`. The signed byte sequence is:

```
"deckymyrig-auth-v1\0" ||
u64_be(timestamp) ||
u16_be(nonce_length) || nonce ||
u16_be(method_length) || uppercase_ascii_method ||
u16_be(path_length) || ascii_path ||
sha256(body)
```

The signature is HMAC-SHA256 with the paired credential. The host accepts a
60-second clock window, compares tags in constant time, and retains accepted
nonces for the clock window. Accepted shutdown nonces are also persisted with
the host identity, so restarting the service cannot make a state-changing replay
valid again; high-frequency status nonces remain bounded in memory. A nonce is
16 random bytes. Invalid authentication attempts are throttled after 20 failures
per clock window, but a valid HMAC is still evaluated so unrelated invalid traffic
cannot lock out an authenticated client. No secret-bearing headers are logged.

After successful request authentication, every success or error response is
HMAC-SHA256 authenticated under a separate response domain. The canonical
response binds the request nonce, path, HTTP status, and exact body digest.
Clients verify this before decoding either success or error Protobuf. Responses
to requests that did not authenticate cannot be authoritative and are rejected
as unauthenticated by a paired client.

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

The service is a GUI-subsystem release executable and never creates interactive
UI. `DeckyMyRigHostControl.exe` is a separate C# WinUI 3 `WinExe`; it maintains a
normal persistent elevated window and obtains state through the local pipe. Windows host
identity, pairing-code state, and the credential are stored together in a
machine-scope DPAPI file restricted to SYSTEM and Administrators. Updates use an
atomic replace operation that works when the destination already exists.

The service binds `0.0.0.0` at the validated TOML port. Network restriction is
provided by the installer firewall rule limited to the Windows Private profile.
Changing the port requires rerunning Setup to synchronize the firewall rule,
restarting the service, and updating the matching Decky device.

## Installer

Inno Setup installs both native executables, the preserved TOML, automatic
service registration, a Private-profile firewall rule, and a normal Start-menu
shortcut for the WinUI application. It was selected for a small native
bootstrapper, stable upgrades via a fixed AppId, elevation, and concise
service/firewall lifecycle scripting. The
config entry uses `onlyifdoesntexist`, preserving custom ports on upgrade.
Credential state and TOML are retained on uninstall for reinstall continuity;
the documentation gives the exact paths for permanent manual removal. The
firewall rule and service are always removed.

Tagged builds Authenticode-sign the service, control application, and completed
installer. The release publishes checksums, CycloneDX SBOMs, a machine-readable
manifest, and provenance attestations. Guided updates constrain the manifest's
installer URL to this repository, stream with a fixed size limit, verify the
SHA-256, validate Windows trust, and pin the signer certificate before launching
Setup with explicit elevation. Setup stops the old service before replacement
and treats service, ACL, firewall, or restart failures as installation failures.
