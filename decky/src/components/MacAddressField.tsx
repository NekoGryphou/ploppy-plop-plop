import { DialogButton, Field, TextField } from "@decky/ui";
import type { JSX } from "react";

/**
* @export
* @desc Props for controller-friendly automatic or manual MAC entry.
*
* @property address - Current normalized or user-entered MAC.
* @property busy - Whether the surrounding form is busy.
* @property canDetect - Whether an address is available for detection.
* @property onAddressChange - Updates the MAC value.
* @property onDetect - Requests backend neighbor discovery.
*/
export interface MacAddressFieldProps {
  address: string;
  busy: boolean;
  canDetect: boolean;
  onAddressChange: (address: string) => void;
  onDetect: () => void;
}

/**
* @public
* @desc Provides mandatory manual MAC entry with optional neighbor-table detection.
*
* @param props - Field state and callbacks.
*
* @returns A native Decky field with a controller-focusable toggle and action.
*/
export function MacAddressField(props: MacAddressFieldProps): JSX.Element {
  return <>
    <Field label="MAC address" childrenContainerWidth="max" bottomSeparator="none" padding="compact">
      <div style={{ display: "flex", alignItems: "center", gap: "10px", width: "100%" }}>
        <TextField
          style={{ width: "100%" }}
          value={props.address}
          disabled={props.busy}
          onChange={(event) => props.onAddressChange(event.target.value)}/>
        <DialogButton aria-label="Detect MAC" style={{ width: "auto", minWidth: "130px" }} disabled={props.busy || !props.canDetect} onClick={props.onDetect}>Detect</DialogButton>
      </div>
    </Field>
  </>;
}
