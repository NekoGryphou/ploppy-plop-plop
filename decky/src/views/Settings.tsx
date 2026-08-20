import { ButtonItem, ConfirmModal, DialogButton, Focusable, Navigation, PanelSection, PanelSectionRow, showModal } from "@decky/ui";
import { useCallback, useEffect, useState, type JSX } from "react";
import { FaChevronLeft, FaPen, FaTrash } from "react-icons/fa";
import { deleteDevice, getDevices } from "../api/backend";
import { DeviceForm } from "../components/DeviceForm";
import type { Device } from "../types";

/** @public @desc Settings route for add/edit/delete and host guidance. @returns Settings content. */
export function Settings(): JSX.Element {
  const [devices, setDevices] = useState<Device[]>([]);
  const [editing, setEditing] = useState<Device | "new">();
  const load = useCallback(async () => setDevices((await getDevices()).devices), []);
  useEffect(() => { void load(); }, [load]);
  const finished = (): void => { setEditing(undefined); void load(); };
  const remove = (device: Device): void => {
    showModal(<ConfirmModal strTitle="Delete PC?" strDescription={`Remove ${device.name} and its pairing credential from this Steam Deck?`} strOKButtonText="Delete" bDestructiveWarning onOK={() => { void deleteDevice(device.id).then(load); }}/>, window);
  };
  if (editing) return <div>
    <Focusable flow-children="horizontal" style={{ display: "flex", alignItems: "center", gap: "12px", marginBottom: "12px" }}>
      <DialogButton
        aria-label="Back to Remote PCs"
        onClick={() => setEditing(undefined)}
        style={{ width: "48px", minWidth: "48px", height: "40px", padding: "8px" }}>
        <FaChevronLeft aria-hidden="true"/>
      </DialogButton>
      <h2 style={{ margin: 0 }}>{editing === "new" ? "Add PC" : "Edit PC"}</h2>
    </Focusable>
    <PanelSection><PanelSectionRow><DeviceForm device={editing === "new" ? undefined : editing} onSaved={finished} onCancel={() => setEditing(undefined)}/></PanelSectionRow></PanelSection>
  </div>;
  return <PanelSection title="Remote PCs">
    {devices.length === 0 && <PanelSectionRow><p>No PCs configured. Install DeckyPowerHost, then pair your first PC.</p></PanelSectionRow>}
    {devices.map((device) => <PanelSectionRow key={device.id}><Focusable flow-children="horizontal" style={{ display: "flex", alignItems: "center", gap: "10px", width: "100%" }}>
      <div style={{ flex: 1 }}><strong>{device.name}</strong><div>{device.address}:{device.port} • {device.mac} • {device.paired ? `Host v${device.host_version ?? "?"}` : "Not paired"}</div></div>
      <DialogButton style={{ width: "48px", minWidth: "48px" }} aria-label={`Edit ${device.name}`} onClick={() => setEditing(device)}><FaPen aria-hidden="true"/></DialogButton>
      <DialogButton style={{ width: "48px", minWidth: "48px" }} aria-label={`Delete ${device.name}`} onClick={() => remove(device)}><FaTrash aria-hidden="true"/></DialogButton>
    </Focusable></PanelSectionRow>)}
    <PanelSectionRow><ButtonItem layout="below" onClick={() => setEditing("new")}>Add PC</ButtonItem></PanelSectionRow>
    <PanelSectionRow><ButtonItem layout="below" onClick={() => Navigation.Navigate("/decky-remote-power/host-setup")}>Host setup instructions</ButtonItem></PanelSectionRow>
  </PanelSection>;
}
