import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { DeviceRow } from "./DeviceRow";
import type { Device } from "../types";

vi.mock("@decky/ui", () => ({ DialogButton: (props: React.ButtonHTMLAttributes<HTMLButtonElement>) => <button {...props}/>, Focusable: (props: React.HTMLAttributes<HTMLDivElement>) => <div {...props}/> }));

const device: Device = { id: "one", name: "Gaming PC", address: "gaming.local", mac: "AA:BB:CC:DD:EE:FF", mac_overridden: false, port: 47991, paired: true };

describe("DeviceRow", () => {
  it.each([["offline", "Start", false], ["online", "Stop", false], ["starting", "…", true], ["stopping", "…", true]] as const)("renders %s", (state, label, disabled) => {
    render(<DeviceRow device={device} status={{ state, message: state }} onAction={vi.fn()}/>);
    expect(screen.getByText("Gaming PC")).toBeVisible();
    const button = screen.getByRole("button");
    expect(button).toHaveTextContent(label);
    expect(button).toHaveProperty("disabled", disabled);
  });
});
