import { DialogButton, Focusable } from "@decky/ui";
import type { CSSProperties, JSX } from "react";
import type { Device, StatusResult } from "../types";

const rowStyle: CSSProperties = { display: "flex", alignItems: "center", justifyContent: "space-between", gap: "12px", padding: "10px 0" };
const labelStyle: CSSProperties = { display: "flex", flexDirection: "column", minWidth: 0 };

/**
* @public
* @desc Controller-friendly compact PC row for Quick Access.
*
* @param props - Device, status, and primary action.
*
* @returns A single-action device row.
*/
export function DeviceRow({ device, status, onAction, onPair }: { device: Device; status?: StatusResult; onAction: () => void; onPair?: () => void }): JSX.Element {
  const state = status?.state ?? "unknown";
  const pairing = status?.pairing ?? (device.paired ? "paired" : "unpaired");
  const pending = state === "starting" || state === "stopping";
  const online = state === "online" || state === "stopping";
  const labels: Record<string, string> = { offline: "○ Offline", starting: "◌ Starting…", online: "● Online", stopping: "◌ Stopping…", unknown: "? Unknown" };
  const pairingLabel = pairing === "unpaired" ? "Not paired" : pairing === "pairing" ? "Pairing…" : pairing === "pairing_failed" ? "Pair again" : pairing === "pairing_expired" ? "Pairing expired" : "";
  const stateLabel = pairingLabel ? `${labels[state]} • ${pairingLabel}` : labels[state];
  const pairingAction = state === "online" && pairing !== "paired" && pairing !== "pairing";
  const actionable = state === "offline" || (state === "online" && pairing === "paired");
  return (
    <Focusable style={rowStyle} flow-children="horizontal">
      <div style={labelStyle}><strong>{device.name}</strong><span aria-live="polite">{stateLabel}</span>{status?.message && status.message !== "Online" && <span>{status.message}</span>}</div>
      <DialogButton style={{ minWidth: "92px", width: "92px" }} disabled={pending || (!actionable && !pairingAction)} onClick={pairingAction ? onPair : onAction} aria-label={`${pairingAction ? "Pair" : online ? "Stop" : "Start"} ${device.name}`}>
        {pending ? "…" : pairingAction ? "Pair" : online ? "Stop" : "Start"}
      </DialogButton>
    </Focusable>
  );
}
