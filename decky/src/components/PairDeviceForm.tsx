import { DialogButton, Field, Focusable, TextField } from "@decky/ui";
import { useState, type JSX } from "react";
import { pairDevice } from "../api/backend";
import type { Device } from "../types";

/**
* @public
* @desc Pairs an already-persisted PC without changing its normal configuration.
*
* @param props - Existing device and completion handlers.
*
* @returns A dedicated pairing form.
*/
export function PairDeviceForm({ device, onPaired, onCancel }: { device: Device; onPaired: () => void; onCancel: () => void }): JSX.Element {
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string>();

  const pair = async (): Promise<void> => {
    const normalized = code.replace(/\s/g, "");
    if (!/^\d{6}$/.test(normalized)) {
      setMessage("Enter the six-digit code shown by DeckyMyRigHostControl.");
      return;
    }
    setBusy(true);
    try {
      const result = await pairDevice(device.id, normalized);
      if (!result.ok) {
        setMessage(result.message ?? "Pairing failed. The PC configuration was kept.");
        return;
      }
      onPaired();
    } catch {
      setMessage("Pairing could not be completed. The PC configuration was kept.");
    } finally {
      setBusy(false);
    }
  };

  return <div>
    <p>Open DeckyMyRigHostControl on {device.name} and generate a pairing code.</p>
    <Field label="Pairing code" childrenContainerWidth="max" bottomSeparator="none" padding="compact">
      <TextField style={{ width: "100%" }} value={code} onChange={(event) => setCode(event.target.value)} disabled={busy} inputMode="numeric"/>
    </Field>
    {message && <p role="alert">{message}</p>}
    <Focusable flow-children="horizontal" style={{ display: "flex", gap: "12px", marginTop: "16px" }}>
      <DialogButton disabled={busy} onClick={onCancel}>Cancel</DialogButton>
      <DialogButton disabled={busy} onClick={() => { void pair(); }}>Pair</DialogButton>
    </Focusable>
  </div>;
}
