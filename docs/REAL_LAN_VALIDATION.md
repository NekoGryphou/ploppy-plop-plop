# Real-LAN acceptance procedure

This procedure is intentionally separate from loopback and CI testing. It
requires a Windows gaming PC, a Steam Deck (or an independent second LAN
machine for the protocol-client steps), and a network where Wake-on-LAN is
allowed. Do not record a PASS unless the stated action was actually observed.

## Record the environment

Before starting, record:

- project commit and installer version;
- Windows version and Windows network profile;
- Decky Loader and plugin versions;
- Windows PC IPv4 address/hostname, configured host port, MAC, and broadcast
  address (do not record pairing codes or credentials);
- whether the Deck and PC are on the same subnet/VLAN.

Example topology:

```text
Windows PC: 192.168.1.42:47991
Steam Deck / second machine: real LAN connection
```

Run `scripts\validate-windows.ps1` first and retain its JSON result. Do not use
`-AllowShutdownTest` during preliminary checks.

## Pairing over the LAN

1. Start `DeckyPowerHost` and open `DeckyPowerHostControl` on Windows.
2. Confirm the control window remains open, reports **Running**, and displays
   the same port configured in `DeckyPowerHost.toml`.
3. Generate a pairing code and leave the window visible.
4. From the Deck, create the PC using name, LAN address, port, and manual MAC,
   with no pairing code. Confirm the saved device says **Not paired**.
5. Enter an incorrect six-digit code in the separate Pair flow. Record FAIL if
   the host pairs, the device is deleted, or its normal configuration changes.
6. Enter the correct current code. Confirm pairing succeeds and Settings shows
   the same existing device as **Paired**; no duplicate device may appear.
7. Attempt to reuse the successful code. It must be rejected.
8. Generate code A and then code B. Code A must fail and code B must succeed.
9. Allow a generated code to reach its displayed expiration. It must fail.
10. Confirm no LAN request or developer client command can retrieve the current
    code. Only the local WinUI/service management pipe may return it.

## Authentication and restart persistence

1. With the PC paired, refresh Decky status and confirm authenticated status
   reports **Online**.
2. Stop and start the Windows service without reinstalling. Authenticated status
   must still succeed with the existing Deck credential.
3. Restart Windows. Confirm the service starts automatically and authenticated
   status succeeds without pairing again.
4. Restart the Steam Deck/Decky Loader. Confirm the device, custom port, MAC,
   host identity, and pairing credential persist and authenticated status
   succeeds.
5. Point the saved address at a different host or use an invalid credential in a
   controlled test. It must not report Online or permit Stop.

## Stop lifecycle

Only run this section on a PC that may safely power off.

1. Confirm the Deck shows **Online** and offers **Stop**.
2. Press Stop once. Confirm **Stopping** appears and the Windows host accepts one
   authenticated shutdown request.
3. Confirm Windows powers off and Decky eventually reports **Offline**.
4. A Stop request from an unpaired device must be unavailable/rejected.

## Wake lifecycle

1. With the Windows PC powered off, confirm the saved device remains visible.
2. Press **Start**. Confirm **Starting** appears; pairing must not be required to
   emit WOL.
3. Confirm the physical PC wakes, Windows boots, and the service automatically
   returns to Running.
4. Confirm Decky transitions to **Online** using authenticated status rather
   than the previous action result.

## Multi-PC isolation

Repeat status refresh with at least two configured PCs while one address is
wrong or unavailable. The healthy PC must remain usable and must not wait for
the broken PC's full timeout before its own state updates.

## Result classifications

For every section record exactly one of:

- **PASS** — executed and succeeded;
- **FAIL** — executed and failed;
- **NOT EXECUTED** — environment or prerequisite unavailable;
- **REQUIRES WINDOWS VALIDATION** — needs Windows runtime evidence;
- **REQUIRES PHYSICAL HARDWARE** — needs the actual Deck, PC, NIC, or LAN.

After testing, run `scripts\collect-diagnostics.ps1`. Preserve its sanitized ZIP
with the checklist results; never add pairing codes or credentials.
