import { DialogButton, Field, TextField, Toggle } from "@decky/ui";
import type { JSX } from "react";

/**
* @export
* @desc Props for controller-friendly automatic or manual MAC entry.
*
* @property address - Current normalized or user-entered MAC.
* @property overridden - Whether manual entry is enabled.
* @property busy - Whether the surrounding form is busy.
* @property canDetect - Whether an address is available for detection.
* @property onAddressChange - Updates the MAC value.
* @property onOverrideChange - Switches automatic/manual mode.
* @property onDetect - Requests backend neighbor discovery.
*/
export interface MacAddressFieldProps {
  address: string;
  overridden: boolean;
  busy: boolean;
  canDetect: boolean;
  onAddressChange: (address: string) => void;
  onOverrideChange: (overridden: boolean) => void;
  onDetect: () => void;
}

/**
* @public
* @desc Lets users auto-detect a MAC from the PC address or override it manually.
*
* @param props - Field state and callbacks.
*
* @returns A native Decky field with a controller-focusable toggle and action.
*/
export function MacAddressField(props: MacAddressFieldProps): JSX.Element {
  return <>
    <Field label="MAC address" childrenContainerWidth="max" bottomSeparator="none" padding="compact">
      {props.overridden
        ? <TextField
          style={{ width: "100%" }}
          value={props.address}
          disabled={props.busy}
          onChange={(event) => props.onAddressChange(event.target.value)}/>
        : <div style={{ display: "flex", alignItems: "center", gap: "10px", width: "100%" }}>
            <span style={{ flex: 1 }}>{props.address || "Not detected"}</span>
            <DialogButton aria-label="Detect MAC" style={{ width: "auto", minWidth: "130px" }} disabled={props.busy || !props.canDetect} onClick={props.onDetect}>Detect</DialogButton>
          </div>}
    </Field>
    <Field label="Enter MAC manually" bottomSeparator="none" padding="compact">
      <Toggle value={props.overridden} disabled={props.busy} onChange={props.onOverrideChange}/>
    </Field>
  </>;
}
