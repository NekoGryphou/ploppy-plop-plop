import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { DeviceRow } from "./DeviceRow";
import type { Device } from "../types";

vi.mock("@decky/ui", () => ({ DialogButton: (props: React.ButtonHTMLAttributes<HTMLButtonElement>) => <button {...props}/>, Focusable: (props: React.HTMLAttributes<HTMLDivElement>) => <div {...props}/> }));

const device: Device = { id: "one", name: "Gaming PC", address: "gaming.local", mac: "AA:BB:CC:DD:EE:FF", mac_overridden: false, port: 47991, paired: true };

describe("DeviceRow", () => {
  it.each([["offline", "Start", false], ["online", "Stop", false], ["starting", "…", true], ["stopping", "…", true]] as const)("renders %s", (state, label, disabled) => {
    render(<DeviceRow device={device} status={{ state, pairing: "paired", message: state }} onAction={vi.fn()}/>);
    expect(screen.getByText("Gaming PC")).toBeVisible();
    const button = screen.getByRole("button");
    expect(button).toHaveTextContent(label);
    expect(button).toHaveProperty("disabled", disabled);
  });

  it("allows Wake-on-LAN for an unpaired device", () => {
    render(<DeviceRow device={{ ...device, paired: false }} status={{ state: "offline", pairing: "unpaired", message: "Offline." }} onAction={vi.fn()}/>);
    expect(screen.getByText("○ Offline • Not paired")).toBeVisible();
    expect(screen.getByRole("button", { name: "Start Gaming PC" })).toBeEnabled();
  });

  it("distinguishes a reachable unpaired host and offers pairing", () => {
    const onPair = vi.fn();
    render(<DeviceRow device={{ ...device, paired: false }} status={{ state: "online", pairing: "unpaired", message: "Pair." }} onAction={vi.fn()} onPair={onPair}/>);
    expect(screen.getByText("● Online • Not paired")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Pair Gaming PC" }));
    expect(onPair).toHaveBeenCalledOnce();
  });

  it("surfaces host update guidance without changing the device action", () => {
    render(<DeviceRow device={device} status={{ state: "online", pairing: "paired", message: "Update the Windows host (installed 1.2.9, plugin 1.3.0)." }} onAction={vi.fn()}/>);
    expect(screen.getByText("Update the Windows host (installed 1.2.9, plugin 1.3.0).")).toBeVisible();
    expect(screen.getByRole("button", { name: "Stop Gaming PC" })).toBeEnabled();
  });
});
