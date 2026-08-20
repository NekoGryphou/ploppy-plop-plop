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
host/           Rust DeckyPowerHost service and Windows installer
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
3. Setup displays a temporary six-digit pairing code. Leave that dialog visible.

The service has no persistent desktop UI after setup and starts automatically
with Windows. Launching `DeckyPowerHost.exe` normally opens an elevated pairing
helper and shows a fresh five-minute code whenever the host is not yet paired.
The Start-menu shortcut **DeckyPowerHost - Pair a Steam Deck** does the same, so
the installer dialog is not the only place the code is available.

### Decky plugin

1. Install the plugin ZIP using Decky Loader's developer/plugin installation
   workflow.
2. Open **Remote PC Power → Settings → Add PC**.
3. Enter a name, hostname/IP, host port, optional broadcast address, and the
   six-digit code shown by Setup. The plugin detects the MAC from the address;
   enable **Enter MAC address manually** if discovery is unavailable or you need
   to override it.
4. Choose **Save and pair**.

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

## Pairing and security

Pairing uses a CSPRNG-generated six-digit code, a five-minute lifetime, five
attempts, SPAKE2, explicit key confirmation, HKDF-SHA256, and ChaCha20-Poly1305.
The code is never sent over the network, and a passive capture does not give an
offline test for guessed codes. A correct exchange creates a random 256-bit
credential that the user never sees.

Every status and shutdown request is authenticated with HMAC-SHA256 over a
canonical length-prefixed timestamp, nonce, HTTP method, path, and body hash. The
host rejects stale clocks, changed bodies/paths, malformed tags, reused nonces,
and repeated failures. HMAC comparisons and pairing confirmation use established
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
- **Pairing fails:** launch `DeckyPowerHost.exe` or the Start-menu pairing
  shortcut to generate a fresh five-minute code. Windows requests elevation
  because pairing state is protected from ordinary processes. If the Deck lost
  an existing credential, use
  `DeckyPowerHost.exe --reset-pairing`, restart the service, and retry promptly.
  Resetting invalidates the previous Deck credential.
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
npm install
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

GitHub Actions uploads the assembled `RemotePCPower/` directory rather than
uploading this ZIP as a file. GitHub's artifact download is therefore directly
usable and does not contain another ZIP inside it.

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

Then build and test everything locally:

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

The Windows CI job runs formatting, Clippy, tests, an MSVC release build, Inno
Setup compilation, and uploads both executables. Automated tests never invoke
real shutdown. Complete [the manual Windows checklist](docs/WINDOWS_VALIDATION.md)
before publishing a release.

## References

The implementation follows current [Decky Loader](https://github.com/SteamDeckHomebrew/decky-loader),
[plugin template](https://github.com/SteamDeckHomebrew/decky-plugin-template),
[`@decky/ui`](https://github.com/SteamDeckHomebrew/decky-frontend-lib), and
[`@decky/api`](https://github.com/SteamDeckHomebrew/loader-api) sources. Host
design follows Microsoft's [service documentation](https://learn.microsoft.com/en-us/windows/win32/services/services),
[`InitiateShutdownW`](https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-initiateshutdownw),
[windows-rs](https://github.com/microsoft/windows-rs), and the
[Protocol Buffers compatibility guidance](https://protobuf.dev/programming-guides/proto3/).
