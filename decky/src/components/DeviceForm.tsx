import { DialogButton, Field, Focusable, TextField } from "@decky/ui";
import { useState, type JSX } from "react";
import { discoverMac, saveDevice } from "../api/backend";
import type { Device, DeviceInput } from "../types";
import { MacAddressField } from "./MacAddressField";

const empty: DeviceInput = { name: "", address: "", mac: "", macOverridden: true, port: "47991", broadcastAddress: "" };

function FormField({ label, children }: { label: string; children: JSX.Element }): JSX.Element {
  return <Field
    label={label}
    childrenContainerWidth="max"
    bottomSeparator="none"
    padding="compact">
    {children}
  </Field>;
}

/**
* @public
* @desc Steam Deck-friendly PC configuration form.
*
* @param props - Existing device and completion handler.
*
* @returns The device editor.
*/
export function DeviceForm({ device, onSaved, onCancel }: { device?: Device; onSaved: () => void; onCancel: () => void }): JSX.Element {
  const [values, setValues] = useState<DeviceInput>(device ? { name: device.name, address: device.address, mac: device.mac, macOverridden: device.mac_overridden, port: String(device.port), broadcastAddress: device.broadcast_address ?? "" } : empty);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string>();
  const set = (key: keyof DeviceInput) => (event: { target: { value: string } }) => setValues((current) => ({ ...current, [key]: event.target.value }));

  const detect = async (): Promise<string | undefined> => {
    if (!values.address.trim()) { setMessage("Enter the PC address before detecting its MAC."); return; }
    try {
      const result = await discoverMac(values.address, values.port);
      setMessage(result.message);
      if (result.ok) { setValues((current) => ({ ...current, mac: result.mac })); return result.mac; }
      return undefined;
    } catch {
      setMessage("MAC could not be detected automatically. Enter it manually.");
      return undefined;
    }
  };

  const submit = async (): Promise<void> => {
    const port = Number(values.port);
    if (!/^\d+$/.test(values.port) || port < 1 || port > 65535) { setMessage("Host port must be a number from 1 to 65535."); return; }
    setBusy(true);
    try {
      const saved = await saveDevice({ ...values, macOverridden: true }, device?.id);
      if (!saved.ok || !saved.device) { setMessage(saved.message ?? "The PC could not be saved."); return; }
      onSaved();
    } catch {
      setMessage("The PC could not be saved. Your existing configuration was not changed.");
    } finally { setBusy(false); }
  };

  return <div>
    <p>DeckyMyRigHost is required for status and shutdown. Starting uses Wake-on-LAN.</p>
    <FormField label="Name"><TextField style={{ width: "100%" }} value={values.name} onChange={set("name")} disabled={busy}/></FormField>
    <FormField label="Address"><TextField style={{ width: "100%" }} value={values.address} onChange={set("address")} disabled={busy}/></FormField>
    <MacAddressField
      address={values.mac}
      busy={busy}
      canDetect={Boolean(values.address.trim())}
      onAddressChange={(mac) => setValues((current) => ({ ...current, mac }))}
      onDetect={() => { setBusy(true); void detect().finally(() => setBusy(false)); }}/>
    <FormField label="Host port"><TextField style={{ width: "100%" }} value={values.port} onChange={set("port")} disabled={busy} inputMode="numeric"/></FormField>
    <FormField label="Broadcast address (optional)"><TextField style={{ width: "100%" }} value={values.broadcastAddress} onChange={set("broadcastAddress")} disabled={busy}/></FormField>
    {message && <p role="alert">{message}</p>}
    <Focusable flow-children="horizontal" style={{ display: "flex", gap: "12px", marginTop: "16px" }}>
      <DialogButton disabled={busy} onClick={onCancel}>Cancel</DialogButton>
      <DialogButton disabled={busy} onClick={() => { void submit(); }}>Save</DialogButton>
    </Focusable>
  </div>;
}
