import { ButtonItem, PanelSection, PanelSectionRow, Navigation } from "@decky/ui";
import type { JSX } from "react";
import { DeviceRow } from "../components/DeviceRow";
import { useDevices } from "../hooks/useDevices";

/** @public @desc Main minimal Quick Access view. @returns Remote PC controls. */
export function QuickAccess(): JSX.Element {
  const { devices, statuses, error, act } = useDevices();
  if (devices.length === 0) return (
    <PanelSection title="Remote PCs"><PanelSectionRow><div><p>No PCs configured.</p><p>Install DeckyPowerHost on your Windows PC, then pair it with your Steam Deck.</p></div></PanelSectionRow><PanelSectionRow><ButtonItem layout="below" onClick={() => Navigation.Navigate("/decky-remote-power/settings")}>Add PC</ButtonItem></PanelSectionRow></PanelSection>
  );
  return <PanelSection>{error && <PanelSectionRow><span>{error}</span></PanelSectionRow>}{devices.map((device) => <PanelSectionRow key={device.id}><DeviceRow device={device} status={statuses[device.id]} onAction={() => { void act(device); }}/></PanelSectionRow>)}</PanelSection>;
}
