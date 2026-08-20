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
export function DeviceRow({ device, status, onAction }: { device: Device; status?: StatusResult; onAction: () => void }): JSX.Element {
  const state = status?.state ?? "unknown";
  const pending = state === "starting" || state === "stopping";
  const online = state === "online" || state === "stopping";
  const labels: Record<string, string> = { offline: "○ Offline", starting: "◌ Starting…", online: "● Online", stopping: "◌ Stopping…", unknown: "? Unknown", authentication_failed: "! Pair again", host_unavailable: "! Host unavailable", update_required: "! Update required" };
  const actionable = state === "online" || state === "offline";
  return (
    <Focusable style={rowStyle} flow-children="horizontal">
      <div style={labelStyle}><strong>{device.name}</strong><span aria-live="polite">{labels[state]}</span></div>
      <DialogButton style={{ minWidth: "92px", width: "92px" }} disabled={pending || !actionable} onClick={onAction} aria-label={`${online ? "Stop" : "Start"} ${device.name}`}>
        {pending ? "…" : online ? "Stop" : "Start"}
      </DialogButton>
    </Focusable>
  );
}
