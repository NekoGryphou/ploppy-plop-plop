# Recovery audit

This audit records the observed baseline before recovery work and the evidence
added afterward. Windows VM results and physical-hardware results are classified
separately.

## Repository state at recovery start

The repository contained:

- a React/TypeScript Decky Quick Access frontend using `@decky/api` and
  `@decky/ui`;
- a Python Decky backend with JSON persistence, Wake-on-LAN, HTTP, hand-written
  Protobuf encoding, pairing, and request authentication;
- a Rust Axum host with config, pairing/authentication, Windows service support,
  shutdown integration, and an Inno Setup installer;
- one shared schema at `proto/decky_my_rig.proto`;
- a browser mockup used for visual inspection.

It did not contain a WinUI 3 control application, a local service-management
IPC, an independent protocol test client, a portable full-chain acceptance
test, a deterministic network-fault topology, or a unified local/CI gate.

## Baseline build and test state

Commands executed before substantial fixes showed:

- Rust formatting and Clippy: PASS;
- Rust tests: 16 passed;
- Decky TypeScript typecheck and production bundle: PASS;
- frontend component tests: 7 passed;
- Python backend tests: 18 passed;
- frontend lint: FAIL because existing non-null assertions violated the active
  lint rules;
- Windows build/runtime, WinUI, installer execution, real shutdown, real WOL,
  and Steam Deck behavior: NOT EXECUTED.

These passing tests were narrow and did not prove the product chain.

## Concrete broken behavior

- Add PC required pairing during creation, so a powered-off PC could not be
  configured for Wake-on-LAN.
- Pairing was coupled to the configuration form instead of being an operation
  on an already-persisted device.
- Pairing-code presentation relied on service/development console behavior;
  there was no persistent WinUI management window or single local management
  API.
- MAC discovery sat in the setup path without sufficient fallback coverage.
- Quick Access could conflate a reachable unpaired host with an offline PC.
- An unpaired WOL attempt reverted from `Starting` to `Offline` on the first
  poll instead of respecting the boot transition timeout.
- No independent client proved the production protocol separately from Decky.
- No test crossed Decky persistence, real TCP/HTTP/Protobuf, production host
  pairing/authentication, mock shutdown, state transitions, and real UDP WOL.
- The Windows credential store used `fs::rename` to overwrite its DPAPI file;
  that operation does not replace an existing destination on Windows, breaking
  later pairing-code and credential saves.
- The WSL UNC-path Windows build copied artifacts into a different output tree
  from the one used by the temporary installer build.

## Suspicious, incomplete, duplicated, or speculative areas

- Pairing-code state had no narrow local owner/interface for an interactive
  control application.
- Test-only mock behavior did not exercise the production Rust host core.
- Network behavior was mostly verified at function boundaries rather than with
  real sockets and captured packets.
- Paired/configured state behavior lacked component-level regression coverage.
- The installer and Windows service code had no Windows CI gate.
- Visual preview execution depended on local browser availability and was not a
  substitute for actual Deck validation.

## Architecture violations at baseline

- Pairing was treated as part of device existence/configuration.
- A usable WinUI 3 pairing application and local-only IPC were absent.
- The host pairing code was available through transient development console
  output rather than a normal persistent Windows UI.
- Platform-independent host behavior was not exposed through a portable
  production-core test path.
- CI and developer commands did not share one authoritative quality gate.

## Recovery order

The recovery follows these sequential gates:

1. config and shared protocol;
2. pairing, authentication, persistence, and replay protection;
3. production host over real TCP with `MockPowerController`;
4. independent production-protocol client;
5. Decky backend against portable/mock hosts;
6. exact Wake-on-LAN bytes and real UDP capture;
7. Decky Quick Access/settings behavior;
8. Rust Windows host, WinUI 3 control app, installer, and safe Windows tests;
9. real LAN, Windows runtime, Steam Deck, shutdown, and physical wake
   acceptance.

Later gates do not convert unavailable Windows or hardware checks into passes.

## Current automated evidence

The primary WSL entry point is `./scripts/check.sh`. It reuses the same scripts
called by CI and currently covers:

- strict TypeScript lint/typecheck, component tests, and production build;
- Python backend unit and protocol-compatible integration tests;
- Rust formatting, Clippy, unit tests, and real-TCP host tests;
- independent client pairing, authenticated status, and mock shutdown;
- persisted create-without-pairing, later pairing of that same device,
  authenticated Stop, host disappearance, and exact captured WOL UDP bytes;
- concurrent paired, unpaired, slow, and unreachable devices on independent
  custom ports;
- generated Python wire descriptors checked against the canonical schema;
- persisted shutdown replay rejection across host restart and authenticated
  pairing identity metadata;
- malformed persisted device data, truncated Protobuf fields, and transactional
  Decky pairing persistence regressions;
- plugin and portable-host builds.

The latest complete local gate executed 23 frontend tests, 35 Python backend
tests, 7 socket integration tests, 25 Rust host tests, 3 Rust test-client tests,
the full lifecycle and concurrent multi-PC scenarios, 5 Toxiproxy network-fault
scenarios, the independent client chain, the cross-language protocol chain, a
Windows-target Rust compile check, and all production builds with zero failures.

Docker Compose/Toxiproxy tests are checked in and mandatory in Linux CI. The
final local gate required Docker and executed all five real network-fault cases:
5 passed and 0 failed.

## Native Windows VM evidence (2026-08-30)

The following was actually executed in the dedicated
`DeckyMyRig-Test-Windows` Hyper-V VM, not inferred from compilation:

- native Windows Rust host tests: 28 passed, 0 failed;
- C# control-model tests: 6 passed, 0 failed;
- release builds of the Windows host, independent client, WinUI 3 application,
  and Inno Setup installer: PASS;
- safe independent-client pair, authenticated status, and mock shutdown over
  real TCP/HTTP/Protobuf: PASS;
- installer upgrade, automatic service registration/start, and preservation of
  the existing custom `48100` port: PASS;
- production Decky Python entrypoint in WSL to the installed Windows Service
  over real WSL-to-VM networking: PASS;
- service-owned pairing code obtained through the real local named pipe, then
  wrong code, correct code, reuse, regenerated-old-code, attempt-limit, and
  real five-minute expiry behavior exercised from Decky: PASS;
- explicitly opted-in production Decky Stop through the installed service and
  real Windows shutdown API, with Hyper-V observing the dedicated VM power off:
  PASS.
- the installed service's live pipe descriptor was read back as
  `O:BAG:SYD:P(A;;FA;;;SY)(A;;FA;;;BA)`: only LocalSystem and BUILTIN\Administrators
  have access; the WinUI application now requests elevation;
- authenticated HTTP errors are response-signed and Decky rejects unsigned or
  tampered errors without invalidating the persisted pairing state.

Earlier VM runs also exercised service stop/start/restart, firewall scope,
WinUI launch/service communication, clean install, upgrade preservation,
uninstall, and DPAPI/named-pipe Windows boundaries. VM-only orchestration and
credentials live under ignored `.local/windows-vm/` and are not public project
artifacts.

## Evidence still requiring physical environments

- actual Steam Deck controller navigation and Decky Loader runtime behavior;
- physical Wake-on-LAN and the complete real-LAN acceptance flow.

Use `docs/WINDOWS_VALIDATION.md`, `scripts/windows/validate-windows.ps1`, and
`scripts/windows/collect-diagnostics.ps1` to gather that evidence without performing a
shutdown by default.
