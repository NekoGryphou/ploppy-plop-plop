import { ButtonItem, ConfirmModal, DialogButton, Focusable, Navigation, PanelSection, PanelSectionRow, showModal } from "@decky/ui";
import { useCallback, useEffect, useState, type JSX } from "react";
import { FaChevronLeft, FaPen, FaTrash } from "react-icons/fa";
import { deleteDevice, getDevices } from "../api/backend";
import { DeviceForm } from "../components/DeviceForm";
import { PairDeviceForm } from "../components/PairDeviceForm";
import type { Device } from "../types";

/**
* @public
* @desc Settings route for add, edit, pairing, deletion, and host guidance.
*
* @returns Settings content.
*/
export function Settings(): JSX.Element {
  const [devices, setDevices] = useState<Device[]>([]);
  const [error, setError] = useState<string>();
  const [editing, setEditing] = useState<Device | "new">();
  const [pairing, setPairing] = useState<Device>();
  const load = useCallback(async () => {
    try {
      setDevices((await getDevices()).devices);
      setError(undefined);
    } catch {
      setError("PC settings could not be loaded. Try reopening this page.");
    }
  }, []);
  useEffect(() => { void load(); }, [load]);
  const finished = (): void => { setEditing(undefined); void load(); };
  const remove = (device: Device): void => {
    const confirmed = async (): Promise<void> => {
      try {
        const result = await deleteDevice(device.id);
        if (!result.ok) { setError(result.message ?? "The PC could not be deleted."); return; }
        await load();
      } catch {
        setError("The PC could not be deleted. Its configuration was kept.");
      }
    };
    showModal(<ConfirmModal strTitle="Delete PC?" strDescription={`Remove ${device.name} and its pairing credential from this Steam Deck?`} strOKButtonText="Delete" bDestructiveWarning onOK={() => { void confirmed(); }}/>, window);
  };
  if (pairing) return <div style={{ paddingBottom: "16px" }}>
    <h2>Pair {pairing.name}</h2>
    <PanelSection><PanelSectionRow><PairDeviceForm device={pairing} onPaired={() => { setPairing(undefined); void load(); }} onCancel={() => setPairing(undefined)}/></PanelSectionRow></PanelSection>
  </div>;
  if (editing) return <div style={{ paddingBottom: "16px" }}>
    <Focusable flow-children="horizontal" style={{ display: "flex", alignItems: "center", gap: "12px", position: "sticky", top: 0, zIndex: 10, padding: "12px 0", background: "#0e141b" }}>
      <DialogButton
        aria-label="Back to PCs"
        onClick={() => setEditing(undefined)}
        style={{ width: "48px", minWidth: "48px", height: "40px", padding: "8px" }}>
        <FaChevronLeft aria-hidden="true"/>
      </DialogButton>
      <h2 style={{ margin: 0 }}>{editing === "new" ? "Add PC" : "Edit PC"}</h2>
    </Focusable>
    <PanelSection><PanelSectionRow><DeviceForm device={editing === "new" ? undefined : editing} onSaved={finished} onCancel={() => setEditing(undefined)}/></PanelSectionRow></PanelSection>
  </div>;
  return <PanelSection title="PCs">
    {error && <PanelSectionRow><p role="alert">{error}</p></PanelSectionRow>}
    {devices.length === 0 && <PanelSectionRow><p>No PCs configured. You can add a powered-off PC now and pair it later.</p></PanelSectionRow>}
    {devices.map((device) => <PanelSectionRow key={device.id}><Focusable flow-children="horizontal" style={{ display: "flex", alignItems: "center", gap: "10px", width: "100%" }}>
      <div style={{ flex: 1 }}><strong>{device.name}</strong><div>{device.address}:{device.port} • {device.mac} • {device.paired ? `Paired • Host v${device.host_version ?? "?"}` : "Not paired"}</div></div>
      <DialogButton style={{ width: device.paired ? "108px" : "76px", minWidth: device.paired ? "108px" : "76px" }} aria-label={`${device.paired ? "Pair again" : "Pair"} ${device.name}`} onClick={() => setPairing(device)}>{device.paired ? "Pair again" : "Pair"}</DialogButton>
      <DialogButton style={{ width: "48px", minWidth: "48px" }} aria-label={`Edit ${device.name}`} onClick={() => setEditing(device)}><FaPen aria-hidden="true"/></DialogButton>
      <DialogButton style={{ width: "48px", minWidth: "48px" }} aria-label={`Delete ${device.name}`} onClick={() => remove(device)}><FaTrash aria-hidden="true"/></DialogButton>
    </Focusable></PanelSectionRow>)}
    <PanelSectionRow><ButtonItem layout="below" onClick={() => setEditing("new")}>Add PC</ButtonItem></PanelSectionRow>
    <PanelSectionRow><ButtonItem layout="below" onClick={() => Navigation.Navigate("/decky-remote-power/host-setup")}>Host setup instructions</ButtonItem></PanelSectionRow>
  </PanelSection>;
}
