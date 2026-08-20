# Manual Windows validation checklist

Run this on a disposable or non-critical x86-64 Windows gaming PC. Real shutdown
is deliberately excluded from automated tests. Record the Windows version, host
commit, installer version, Decky Loader version, and network profile first.

- [ ] Build `DeckyPowerHost.exe` and `DeckyPowerHost-Setup.exe` on native Windows.
- [ ] Run Setup and confirm the normal UAC elevation prompt explains publisher/admin access.
- [ ] Confirm `C:\Program Files\DeckyPowerHost\DeckyPowerHost.toml` exists beside the EXE.
- [ ] Confirm it contains `port = 47991` on a fresh install.
- [ ] Confirm `DeckyPowerHost` is registered as an automatic LocalSystem service.
- [ ] Confirm the service starts and reports Running.
- [ ] Confirm no console window appears for the service.
- [ ] Confirm TCP `0.0.0.0:47991` is listening.
- [ ] Confirm the `DeckyPowerHost` inbound firewall rule is TCP 47991 and Private only.
- [ ] Confirm the rule does not allow Public-profile traffic.
- [ ] Confirm Setup displays a six-digit temporary pairing code without exposing a long-term secret.
- [ ] Launch `DeckyPowerHost.exe` as a standard user, approve UAC, and confirm a
      fresh pairing code is shown without a logging panic.
- [ ] Confirm the Start-menu **DeckyPowerHost - Pair a Steam Deck** shortcut opens
      the same pairing helper after installation.
- [ ] Pair from Decky on port 47991 and confirm authenticated status succeeds.
- [ ] Confirm an incorrect pairing code fails and does not leave the host paired.
- [ ] Change TOML to `port = 48100`, rerun Setup to synchronize the firewall rule, then restart the service.
- [ ] Update that device to port 48100 in Decky.
- [ ] Confirm authenticated status and new pairing both work on port 48100.
- [ ] Confirm a device configured with the old/wrong port reports an actionable unavailable error.
- [ ] Confirm invalid HMAC, changed body/path, stale timestamp, malformed signature, and replayed nonce are rejected.
- [ ] Confirm repeated authentication failures receive rate limiting.
- [ ] Confirm `DeckyPowerHost.exe --dev --mock-shutdown --config <path>` uses real HTTP/Protobuf/auth but never shuts down.
- [ ] Confirm authenticated mock shutdown logs acceptance and the mock safety message without credentials.
- [ ] On a safe test PC, confirm real shutdown uses the Windows API and powers off.
- [ ] Wake it using Decky WOL and confirm the service starts automatically after boot.
- [ ] Confirm Decky moves Offline → Starting → Online.
- [ ] Shut down from Decky and confirm Online → Stopping → Offline.
- [ ] Reboot the Deck and PC and confirm device, credential, host ID, and custom port persistence.
- [ ] Install a newer Setup build and confirm existing TOML is byte-for-byte preserved.
- [ ] Confirm the custom port survives the upgrade and Setup synchronizes the Private firewall rule.
- [ ] Exercise a host/plugin protocol mismatch and confirm “host update required” guidance.
- [ ] Uninstall and confirm service and firewall rule removal.
- [ ] Confirm TOML and DPAPI credentials remain, as documented, for reinstall continuity.
- [ ] Delete retained `C:\Program Files\DeckyPowerHost\DeckyPowerHost.toml` and `%ProgramData%\DeckyPowerHost` manually when permanent credential removal is desired.
