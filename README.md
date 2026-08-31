# Decky Remote PC Power

Start and shut down one or more Windows gaming PCs from Decky Loader's Quick
Access panel. Daily use stays intentionally small: choose a PC and press **Start**
or **Stop**.

```text
Start → Wake-on-LAN
Stop  → authenticated DeckyPowerHost request → Windows shutdown API
```

Everything stays on the local network. There are no accounts, cloud services,
telemetry, or generic remote-command endpoints.

## Repository layout

```text
decky/          Decky plugin frontend, backend, tests, and UI preview
host/           Rust service, WinUI control app, and Windows installer
proto/          Shared Protocol Buffers contract
docs/           Architecture and Windows validation documentation
scripts/        Repository build entry points
out/host/       Windows host executable and installer
out/plugin/     Installable Decky plugin ZIP
out/tests/      UI screenshots and other generated test evidence
```

The plugin and Windows host are deliberately self-contained. Only the versioned
Protobuf contract is shared between them.

## Requirements

Steam Deck:

- [Decky Loader](https://github.com/SteamDeckHomebrew/decky-loader).

Windows PC:

- x86-64 Windows 10 or Windows 11;
- a Wake-on-LAN-capable network adapter and LAN;
- the produced `DeckyPowerHost-Setup.exe`.

No Python required. No Node.js required. No Java required. No SSH required. No
SSH keys required. No Windows password is stored on the Steam Deck.

## Install for normal use

### Windows host

1. Download `DeckyPowerHost-Setup.exe` from the project release or CI artifact.
2. Run it and accept elevation. Elevation is needed only to copy into Program
   Files, register the automatic Windows service, protect its data directory, and
   create a Private-profile firewall rule.
3. Setup starts the headless service and offers to open `DeckyPowerHostControl`.
4. Approve the control application's elevation prompt and keep its normal window
   open while pairing. It displays the service state, configured port, pairing
   state, six-digit code, expiration, regeneration action, and connection errors.

`DeckyPowerHost.exe` never presents interactive UI. Pairing is owned by the
service and exposed to the elevated WinUI 3 control application through a
narrow, local-only named pipe restricted to LocalSystem and Administrators. The
LAN API accepts pairing exchanges but cannot reveal the current code.

### Decky plugin

1. Install the plugin ZIP using Decky Loader's developer/plugin installation
   workflow.
2. Open **Remote PC Power → Settings → Add PC**.
3. Enter a name, hostname/IP, host port, MAC address, and optional broadcast
   address, then choose **Save**. The PC may be powered off and no pairing code
   or network connection is required.
4. The saved PC appears as **Not paired** and can already be started with WOL.
5. When the PC is awake, choose **Pair**, open `DeckyPowerHostControl` on
   Windows, generate a code, and enter that code in the separate pairing form.

Repeat for each PC. Every PC keeps an independent address, port, MAC, host
identity, and credential.

## Host configuration

The only ordinary host setting is beside the executable:

```text
C:\Program Files\DeckyPowerHost\DeckyPowerHost.toml
```

```toml
port = 47991
```

Valid ports are 1 through 65535. A missing file or missing `port` uses the
documented 47991 default. Port 0, values above 65535, non-numeric values, unknown
keys, and malformed TOML stop startup with a clear error. A port already in use
also prevents startup instead of selecting a random port.

To change a port:

1. edit TOML as Administrator;
2. rerun the same Setup executable—this preserves TOML and recreates the narrow
   Private firewall rule for its current port;
3. restart the `DeckyPowerHost` service;
4. update only that PC's **Host port** in Decky.

Reinstallation/upgrades use Inno Setup's `onlyifdoesntexist` rule and never
replace existing TOML. You do not need to reinstall merely to change the port;
rerunning Setup is the supported firewall synchronization mechanism.

`DeckyPowerHostControl` shows the running host version and provides a guided
update. It accepts installers only from this repository's HTTPS release path,
enforces the release-manifest SHA-256, validates the Windows Authenticode trust
chain, and pins the release signing-certificate thumbprint embedded at build
time before requesting elevation. Running the verified Setup upgrades the
service in place.
The machine DPAPI identity in `%ProgramData%\DeckyPowerHost` is outside the
installation directory, so host upgrades preserve the host UUID and pairing
credential. Decky Loader manages plugin package updates; the plugin reopens the
same versioned settings and credential files after replacement. Compatible host
and plugin updates therefore do not require pairing again.

Both components advertise their `X.Y.Z` release version. A major-version
difference is treated as incompatible, a minor-version difference shows which
component to update, and patch-only differences remain silent. The plugin shows
the guidance beside the affected PC; the Windows control application shows the
last authenticated plugin version it observed. Version reporting is additive
metadata and never rotates or deletes the pairing credential.
The last authenticated plugin version is stored with the DPAPI-protected host
identity, retaining update guidance across service restarts without trusting
unauthenticated LAN metadata.

## Pairing and security

Pairing uses a CSPRNG-generated six-digit code, a five-minute lifetime, SPAKE2,
explicit key confirmation, HKDF-SHA256, and ChaCha20-Poly1305. Failed key
confirmations are limited to five per source and code. Unconfirmed exchanges are bounded
globally and per source address so one LAN client cannot consume every slot.
The code is never sent over the network, and a passive capture does not give an
offline test for guessed codes. A correct exchange creates a random 256-bit
credential that the user never sees.

Every status and shutdown request is authenticated with HMAC-SHA256 over a
canonical length-prefixed timestamp, nonce, HTTP method, path, and body hash. The
host rejects stale clocks, changed bodies/paths, malformed tags, reused nonces,
and repeated failures. Accepted shutdown nonces survive service restarts for the
replay window. Pairing credential encryption also authenticates the returned host
identity metadata. HMAC comparisons and pairing confirmation use established
constant-time implementations. The paired stable host UUID prevents an address
being silently reassigned to another PC.

Windows stores host identity and credentials under
`%ProgramData%\DeckyPowerHost\credentials.dpapi`, encrypted using machine-scope
DPAPI and restricted to SYSTEM/Administrators. SteamOS provides no universally
available OS keyring for Decky plugins, so Deck credentials are kept outside
frontend-visible settings in a mode-0600 backend file. This limits exposure but
does not protect against another process already running as the `deck` user.
Secrets are never displayed or logged.

Operational host logs are written to
`%ProgramData%\DeckyPowerHost\DeckyPowerHost.log`. They include version, protocol,
config path, selected port, authentication rejection categories, pairing success,
and shutdown acceptance/failure, but never codes, credentials, or authorization
headers.

HTTP is used without TLS. SPAKE2 secures secret establishment and HMAC secures
command integrity/authenticity without invisible certificate lifecycle problems.
Traffic metadata and non-secret status information are not confidential from a
LAN observer. The listener binds the LAN interfaces; the installer limits access
to the Windows Private profile, and authentication remains mandatory. See
[architecture and threat model](docs/ARCHITECTURE.md).

## Wake-on-LAN

Enable Wake-on-LAN in BIOS/UEFI and in the Windows network adapter's Power
Management/Advanced settings. Wired Ethernet is most reliable. The PC must keep
its NIC powered while shut down. If subnet broadcast is not sufficient, enter
the LAN's explicit broadcast address in Decky. The backend sends standard magic
packets on UDP ports 9 and 7; DeckyPowerHost is not involved in startup.

## Troubleshooting

- **PC does not wake:** verify BIOS/NIC WOL, MAC, wired power state, and optional
  broadcast address. Automatic MAC detection requires the PC to be awake and on
  the same IPv4 LAN during setup; manual override accepts common MAC formats.
- **Host unavailable / wrong port:** verify the PC is awake, the service is
  Running, TOML and the per-PC Decky port match, and Setup synchronized the
  Private firewall rule.
- **Port already in use:** choose another valid TOML port, rerun Setup, restart
  the service, and update Decky.
- **Pairing fails:** open `DeckyPowerHostControl`, verify the service is Running,
  and generate a fresh five-minute code. The device configuration remains saved
  after wrong or expired codes. Generating a new code invalidates the previous
  code; successful re-pairing replaces the previous Deck credential. A lost
  final response is retried once with the same pairing exchange.
- **Authentication fails:** use **Pair again**; an address may point to another
  host or protected state may have been reset.
- **PC appears offline:** authenticated host status is authoritative; firewalls,
  DNS, and a stopped service can resemble a powered-off PC.
- **Shutdown rejected:** inspect the Windows service log and validate the
  LocalSystem shutdown privilege on Windows. No shell fallback exists.
- **Host version incompatible:** update DeckyPowerHost with the current Setup.
- **Invalid host config:** correct TOML syntax and port range, then restart.

## Developer setup

End users do not need any development tools. Contributors use only mainstream
toolchains from their official sources:

- [Git](https://git-scm.com/downloads)
- [Node.js 22 LTS](https://nodejs.org/en/download)
- [Python 3.11](https://www.python.org/downloads/) with pip for Decky backend tests
- [pnpm 9](https://pnpm.io/installation) or npm bundled with Node
- [Rustup](https://rustup.rs/) (stable Rust, rustfmt, and Clippy)
- [Decky CLI](https://github.com/SteamDeckHomebrew/cli)
- [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/)
  with “Desktop development with C++” for native MSVC builds
- [Inno Setup](https://jrsoftware.org/isdl.php) for the Windows installer
- [Windows Package Manager](https://learn.microsoft.com/windows/package-manager/winget/)
  for the optional one-command Windows toolchain setup

The Rust build downloads a pinned vendored `protoc`; no system Protobuf compiler
is required. The protocol source remains [decky_power.proto](proto/decky_power.proto).

### Build and test the Decky plugin on Linux/WSL

```bash
git clone <repository-url>
cd decky-my-rig
cd decky
npm ci
npm run backend:deps
npm run build
npm test
npm run zip
```

The frontend artifact is `decky/dist/index.js`; all distributable artifacts are
collected in the repository-level `out/` directory. The installable plugin is
`out/plugin/RemotePCPower.zip`. The ZIP command checks that pinned Python dependencies
were bundled first. For live Decky development, install and
configure the official Decky CLI, then use its plugin build/deploy commands from
the repository root as documented by the
[official template](https://github.com/SteamDeckHomebrew/decky-plugin-template)
and [development guide](https://wiki.deckbrew.xyz/en/loader-dev/development).

GitHub Actions uploads `out/plugin/RemotePCPower.zip` as the portable plugin
artifact. Extract that archive before using Decky Loader's plugin installation
workflow.

### Review the UI without a Steam Deck

```bash
npx playwright install --with-deps chromium
npm run visual
```

Run this from `decky/`. It starts a localhost-only Vite preview, renders the real
Quick Access row and form components with mock PCs at Deck-sized dimensions, and
writes `out/tests/decky-ui.png`. It never contacts a host or handles credentials.

### Test the portable Rust host safely

```bash
cd host
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
printf 'port = 47991\n' > DeckyPowerHost.toml
cargo run -- --dev --mock-shutdown --config DeckyPowerHost.toml
```

Development mode uses real HTTP, Protobuf, SPAKE2, HMAC, and persistence, but its
power controller only records the request. It refuses `--dev` without
`--mock-shutdown`.

### Primary local portable pipeline

From WSL/Linux, the checked-in commands shared with CI are:

```bash
./scripts/build.sh
./scripts/test.sh
./scripts/test-e2e.sh
./scripts/check.sh
```

`check.sh` is the primary gate. It runs static checks, frontend/backend tests,
Rust formatting and Clippy, production builds, real-socket pairing/status/mock
shutdown, the complete persisted lifecycle test, real UDP WOL capture, and
plugin packaging. The optional `.devcontainer` defines Node 22, pnpm 9.15.9,
Rust 1.98.0, Protobuf tooling, Docker Engine, and Docker Compose. When Docker is
available, `scripts/network/test-toxiproxy.sh` runs the production portable hosts through
Toxiproxy. Without Docker, set `TOXIPROXY_SERVER` to an official executable
Toxiproxy 2.12.0 server binary to run the same faults against an isolated local
production-host process. If neither topology is available, the result is
reported as `NOT EXECUTED`, not as a pass.

### Build Windows host and installer (native Windows)

Open **x64 Native Tools Command Prompt for VS 2022**, then:

```bat
git clone <repository-url>
cd decky-my-rig
rustup target add x86_64-pc-windows-msvc
powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1
```

Artifacts:

```text
out\host\DeckyPowerHost.exe
out\control\DeckyPowerHostControl.exe
out\host\DeckyPowerHost-Setup.exe
```

### Build everything locally from WSL

GitHub Actions is a clean-environment verifier, not a build requirement. From
the repository root, install the official Windows build tools once:

```bash
./scripts/setup-local.sh
```

The setup script uses the official tools provided through `winget` packages
[`Rustlang.Rustup`](https://rustup.rs/),
[`Microsoft.VisualStudio.2022.BuildTools`](https://visualstudio.microsoft.com/downloads/),
and [`JRSoftware.InnoSetup`](https://jrsoftware.org/isdl.php). It installs the
stable Rust MSVC target, rustfmt, and Clippy. It does not install or start
DeckyPowerHost, create a service, alter the firewall, or execute the installer.

Windows interoperability is optional and not part of the portable gate. To run
the native Windows build from WSL when Windows build tools are deliberately
available:

```bash
./scripts/build-local.sh
```

This performs a reproducible Decky dependency install, frontend/backend tests,
plugin packaging, UI screenshot validation, native Windows tests and release
linking, and Inno Setup compilation. Use `./scripts/build-local.sh --skip-ui`
only when Chromium/Playwright system dependencies are intentionally unavailable.
All final output remains under:

```text
out/host/
out/plugin/
out/tests/
```

On a normal native Windows clone, run the same host build directly:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\setup-windows-build.ps1
powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1
```

Do not install the service, modify the firewall, or test real shutdown from WSL.
WSL can type-check Windows-only Rust code, but only native Windows and the Windows
CI runner validate linking and installer compilation.

## Uninstall and retained data

Uninstall always stops/deletes the service, removes the binary, and deletes the
firewall rule. TOML and DPAPI pairing state are retained deliberately for easy
reinstallation. For permanent removal, delete the retained TOML and
`%ProgramData%\DeckyPowerHost` as Administrator after uninstalling.

## Validation

Independent reviewers should begin with [the reviewer guide](docs/REVIEW_GUIDE.md),
which maps trust boundaries, security-critical files, reproducible gates, and
the remaining physical-hardware evidence boundary.

The Windows CI job runs safe tests, an MSVC release build, WinUI model tests,
WinUI 3 publish, Inno Setup compilation, and uploads the artifacts. This is
distinct from real-host validation. Run `scripts/windows/validate-windows.ps1` and
`scripts/windows/collect-diagnostics.ps1` on the gaming PC, then complete the
[manual Windows checklist](docs/WINDOWS_VALIDATION.md) and
[real-LAN acceptance procedure](docs/REAL_LAN_VALIDATION.md) before publishing.

## References

The implementation follows current [Decky Loader](https://github.com/SteamDeckHomebrew/decky-loader),
[plugin template](https://github.com/SteamDeckHomebrew/decky-plugin-template),
[`@decky/ui`](https://github.com/SteamDeckHomebrew/decky-frontend-lib), and
[`@decky/api`](https://github.com/SteamDeckHomebrew/loader-api) sources. Host
design follows Microsoft's [service documentation](https://learn.microsoft.com/en-us/windows/win32/services/services),
[`InitiateShutdownW`](https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-initiateshutdownw),
[windows-rs](https://github.com/microsoft/windows-rs), and the
[Protocol Buffers compatibility guidance](https://protobuf.dev/programming-guides/proto3/).
