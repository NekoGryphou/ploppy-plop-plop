# Remaining Windows and physical validation checklist

## Automated dedicated-VM evidence

Executed on 2026-08-30:

- [x] Native Windows host, protocol client, WinUI 3 app, and installer build.
- [x] 30 Windows Rust tests and 6 C# control-model tests.
- [x] Install/upgrade with an Automatic, Running LocalSystem service.
- [x] Custom port `48100` preserved through installer upgrade.
- [x] Local named-pipe code generation and Windows DPAPI persistence paths.
- [x] WSL production Decky backend to installed VM service over real networking.
- [x] Wrong/correct/reused/regenerated/attempt-limited/expired pairing cases.
- [x] Authenticated status and mock-shutdown protocol path.
- [x] Explicit real Windows shutdown API test; Hyper-V observed the dedicated VM
  power off.
- [x] Service stop/start/restart, firewall, installer upgrade/uninstall, and
  WinUI service communication in earlier dedicated-VM runs.

These checks do not prove behavior on a physical gaming PC or Steam Deck.

The current working tree was revalidated on the dedicated VM on 2026-08-30:
native compilation, all 30 Rust tests, all 6 C# tests, self-contained WinUI
publish, and Inno Setup compilation passed. A silent upgrade installed the exact
built host binary, preserved the custom-port TOML and DPAPI credential file
byte-for-byte, retained the Private-only firewall rule, and returned the service,
listener, and management pipe to Running state. Production pairing and
authenticated status succeeded before and after a service restart. The safe
Windows pair/status/mock-shutdown E2E also passed. The opt-in real shutdown was
not repeated during this revalidation.

## Remaining physical acceptance

This checklist applies only to native physical-hardware acceptance.

Run this on a disposable or non-critical x86-64 Windows gaming PC. Real shutdown
is deliberately excluded from automated tests. Record the Windows version, host
commit, installer version, Decky Loader version, and network profile first.

- [ ] Run Setup and confirm the normal UAC elevation prompt explains publisher/admin access.
- [ ] Confirm `C:\Program Files\DeckyMyRigHost\DeckyMyRigHost.toml` exists beside the EXE.
- [ ] Confirm it contains `port = 47991` on a fresh install.
- [ ] Confirm no console window appears for the service.
- [ ] Confirm TCP `0.0.0.0:47991` is listening.
- [ ] Confirm the inbound firewall rule follows the configured port and is Private only.
- [ ] Confirm the rule does not allow Public-profile traffic.
- [ ] Launch the Start-menu **Decky My Rig Host** shortcut and confirm a persistent normal WinUI 3 window remains open.
- [ ] Confirm service state and configured port are visible.
- [ ] Confirm a six-digit pairing code and expiration countdown remain visible.
- [ ] Confirm **Generate new code** replaces the code and invalidates the old one.
- [ ] Confirm paired state and service connection errors are displayed in the window.
- [ ] Confirm launching the service executable never opens a pairing terminal, console, or message box.
- [ ] Confirm the management named pipe rejects remote and non-administrator clients, while the elevated control app can connect.
- [ ] Pair from the physical Deck and confirm authenticated status succeeds.
- [ ] Change TOML to `port = 48100`, rerun Setup to synchronize the firewall rule, then restart the service.
- [ ] Update that device to port 48100 in Decky.
- [ ] Confirm authenticated status and new pairing both work on port 48100.
- [ ] Confirm a device configured with the old/wrong port reports an actionable unavailable error.
- [ ] Confirm `DeckyMyRigHost.exe --dev --mock-shutdown --config <path>` uses real HTTP/Protobuf/auth but never shuts down.
- [ ] On a safe physical test PC, confirm real shutdown uses the Windows API and powers off.
- [ ] Wake it using Decky WOL and confirm the service starts automatically after boot.
- [ ] Confirm Decky moves Offline → Starting → Online.
- [ ] Shut down from Decky and confirm Online → Stopping → Offline.
- [ ] Reboot the Deck and PC and confirm device, credential, host ID, and custom port persistence.
- [ ] Install a newer Setup build and confirm existing TOML is byte-for-byte preserved.
- [ ] Confirm the custom port survives the upgrade and Setup synchronizes the Private firewall rule.
- [ ] Exercise a host/plugin protocol mismatch and confirm “host update required” guidance.
- [ ] Uninstall and confirm service and firewall rule removal.
- [ ] Confirm TOML and DPAPI credentials remain, as documented, for reinstall continuity.
- [ ] Delete retained `C:\Program Files\DeckyMyRigHost\DeckyMyRigHost.toml` and `%ProgramData%\DeckyMyRigHost` manually when permanent credential removal is desired.

Before manual checks, run `scripts\windows\validate-windows.ps1` without destructive
flags. Use `scripts\windows\collect-diagnostics.ps1` to produce a sanitized archive for
review. Neither script includes pairing codes or credentials, and neither runs
a real shutdown by default.
