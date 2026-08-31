# Reviewer guide

This repository is a pre-release Decky Loader plugin plus a Rust Windows
Service and C#/WinUI 3 management application. There is intentionally one
strict application protocol version (`v1`); breaking pre-release changes do not
carry compatibility branches.

## Start here

- `README.md`: product behavior, build commands, and user-facing security model.
- `docs/ARCHITECTURE.md`: components, trust boundaries, and protocol flow.
- `SECURITY.md`: threat model, secret storage, and dependency policy.
- `proto/decky_my_rig.proto`: canonical wire contract.
- `docs/RECOVERY_AUDIT.md`: executed evidence versus physical-hardware gaps.

Public reproducible gates:

```bash
./scripts/build.sh
./scripts/test.sh
./scripts/test-e2e.sh
REQUIRE_DOCKER=1 ./scripts/check.sh
```

The final command is the authoritative Linux/WSL gate. It requires Docker for
real Toxiproxy faults. Native Windows uses:

```powershell
.\scripts\windows\build-windows.ps1
.\scripts\windows\test-windows.ps1
```

VM orchestration and credentials are deliberately absent from the public tree;
they are local test infrastructure, not product source.

## Security-critical review map

- `host/src/pairing.rs`: code lifetime/attempts, SPAKE2, key confirmation,
  credential generation, and AEAD binding.
- `host/src/auth.rs`: canonical requests, HMAC verification, replay window, and
  response authentication.
- `host/src/server.rs`: route validation order, authenticated success/error
  responses, shutdown nonce persistence, and LAN pairing surface.
- `host/src/management_ipc.rs`: local-only named-pipe framing and Administrator/
  LocalSystem DACL.
- `host/src/storage/windows.rs`: DPAPI and atomic credential replacement.
- `host/installer/DeckyMyRigHost.iss`: service identity, ProgramData ACL, and
  Private-profile firewall rule.
- `decky/py_modules/decky_my_rig/client.py`: production HTTP/Protobuf client,
  pairing, and mandatory success/error response verification.
- `decky/py_modules/decky_my_rig/store.py`: separation and mode-0600 persistence
  of Deck-side credentials.
- `tools/decky-my-rig-test/src/main.rs`: independent protocol implementation.

High-value regressions are in `host/src/*` unit modules,
`decky/tests/backend/test_protocol.py`, `decky/tests/e2e/`, and
`decky/tests/integration/production_host_client.py`. Test hosts reuse the real
host core and routes; only `PowerController` is mocked in ordinary automation.

## Evidence boundary

Portable real-socket lifecycle, exact UDP WOL emission, multi-PC independence,
network faults, native Windows builds/tests, installed-service communication,
and a dedicated-VM real shutdown have been executed. Actual Decky Loader UI and
controller navigation on a Steam Deck, physical NIC wake, firmware behavior,
and a real home LAN still require physical acceptance.
