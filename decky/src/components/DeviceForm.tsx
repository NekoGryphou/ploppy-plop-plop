import { DialogButton, Field, Focusable, TextField } from "@decky/ui";
import { useState, type JSX } from "react";
import { discoverMac, pairDevice, saveDevice } from "../api/backend";
import type { Device, DeviceInput } from "../types";
import { MacAddressField } from "./MacAddressField";

const empty: DeviceInput = { name: "", address: "", mac: "", macOverridden: false, port: "47991", broadcastAddress: "" };

/**
* @public
* @desc Steam Deck-friendly add/edit and pairing form.
*
* @param props - Existing device and completion handler.
*
* @returns The device editor.
*/
export function DeviceForm({ device, onSaved, onCancel }: { device?: Device; onSaved: () => void; onCancel: () => void }): JSX.Element {
  const [values, setValues] = useState<DeviceInput>(device ? { name: device.name, address: device.address, mac: device.mac, macOverridden: device.mac_overridden, port: String(device.port), broadcastAddress: device.broadcast_address ?? "" } : empty);
  const [pairingCode, setPairingCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string>();
  const set = (key: keyof DeviceInput) => (event: { target: { value: string } }) => setValues((current) => ({ ...current, [key]: event.target.value }));

  const detect = async (): Promise<string | undefined> => {
    if (!values.address.trim()) { setMessage("Enter the PC address before detecting its MAC."); return; }
    const result = await discoverMac(values.address, values.port);
    setMessage(result.message);
    if (result.ok) { setValues((current) => ({ ...current, mac: result.mac })); return result.mac; }
    return undefined;
  };

  const submit = async (): Promise<void> => {
    const port = Number(values.port);
    if (!/^\d+$/.test(values.port) || port < 1 || port > 65535) { setMessage("Host port must be a number from 1 to 65535."); return; }
    if (!device && !/^\d{6}$/.test(pairingCode)) { setMessage("Enter the six-digit code shown by DeckyPowerHost."); return; }
    setBusy(true);
    try {
      let submitted = values;
      if (!values.macOverridden) {
        const mac = await detect();
        if (!mac) return;
        submitted = { ...values, mac };
      }
      const saved = await saveDevice(submitted, device?.id);
      if (!saved.ok || !saved.device) { setMessage(saved.message ?? "The PC could not be saved."); return; }
      if (pairingCode) {
        if (!/^\d{6}$/.test(pairingCode)) { setMessage("Enter the six-digit code shown by DeckyPowerHost."); return; }
        const paired = await pairDevice(saved.device.id, pairingCode);
        if (!paired.ok) { setMessage(paired.message ?? "Pairing failed."); return; }
      }
      onSaved();
    } finally { setBusy(false); }
  };

  return <div>
    <p>DeckyPowerHost is required for status and shutdown. Starting uses Wake-on-LAN.</p>
    <Field label="Name"><TextField value={values.name} onChange={set("name")} disabled={busy}/></Field>
    <Field label="Address"><TextField value={values.address} onChange={set("address")} disabled={busy}/></Field>
    <MacAddressField
      address={values.mac}
      overridden={values.macOverridden}
      busy={busy}
      canDetect={Boolean(values.address.trim())}
      onAddressChange={(mac) => setValues((current) => ({ ...current, mac }))}
      onOverrideChange={(macOverridden) => setValues((current) => ({ ...current, macOverridden }))}
      onDetect={() => { setBusy(true); void detect().finally(() => setBusy(false)); }}/>
    <Field label="Host port"><TextField value={values.port} onChange={set("port")} disabled={busy} inputMode="numeric"/></Field>
    <Field label="Broadcast address (optional)"><TextField value={values.broadcastAddress} onChange={set("broadcastAddress")} disabled={busy}/></Field>
    <Field label="Pairing code (required for a new PC)"><TextField value={pairingCode} onChange={(event) => setPairingCode(event.target.value)} disabled={busy} inputMode="numeric"/></Field>
    {message && <p role="alert">{message}</p>}
    <Focusable flow-children="horizontal" style={{ display: "flex", gap: "12px", marginTop: "16px" }}>
      <DialogButton disabled={busy} onClick={onCancel}>Cancel</DialogButton>
      <DialogButton disabled={busy} onClick={() => { void submit(); }}>{!device || pairingCode ? "Save and pair" : "Save"}</DialogButton>
    </Focusable>
  </div>;
}
