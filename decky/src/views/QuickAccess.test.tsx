import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useDevices } from "../hooks/useDevices";
import type { Device } from "../types";
import { QuickAccess } from "./QuickAccess";

const navigate = vi.fn();

vi.mock("@decky/ui", () => ({
  ButtonItem: (props: React.ButtonHTMLAttributes<HTMLButtonElement>) => <button {...props}/>,
  DialogButton: (props: React.ButtonHTMLAttributes<HTMLButtonElement>) => <button {...props}/>,
  Focusable: (props: React.HTMLAttributes<HTMLDivElement>) => <div {...props}/>,
  Navigation: { Navigate: (...arguments_: unknown[]) => navigate(...arguments_) },
  PanelSection: ({ children }: React.PropsWithChildren) => <section>{children}</section>,
  PanelSectionRow: ({ children }: React.PropsWithChildren) => <div>{children}</div>,
}));
vi.mock("../hooks/useDevices", () => ({ useDevices: vi.fn() }));

const device: Device = {
  id: "gaming",
  name: "Gaming PC",
  address: "gaming-pc.local",
  port: 47991,
  mac: "AA:BB:CC:DD:EE:FF",
  mac_overridden: true,
  paired: false,
};

describe("QuickAccess", () => {
  beforeEach(() => {
    navigate.mockReset();
    vi.mocked(useDevices).mockReset();
  });

  it("does not imply pairing is required before adding a powered-off PC", () => {
    vi.mocked(useDevices).mockReturnValue({ devices: [], statuses: {}, error: undefined, refresh: vi.fn(), act: vi.fn() });
    render(<QuickAccess/>);
    expect(screen.getByText("Add a PC using its address and MAC. It can be powered off and paired later.")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Add PC" }));
    expect(navigate).toHaveBeenCalledWith("/decky-remote-power/settings");
  });

  it("keeps Wake-on-LAN available while an unpaired host is offline", () => {
    const act = vi.fn();
    vi.mocked(useDevices).mockReturnValue({ devices: [device], statuses: { gaming: { state: "offline", pairing: "unpaired", message: "Offline" } }, error: undefined, refresh: vi.fn(), act });
    render(<QuickAccess/>);
    fireEvent.click(screen.getByRole("button", { name: "Start Gaming PC" }));
    expect(act).toHaveBeenCalledWith(device);
  });

  it("distinguishes a reachable unpaired host from offline and offers Pair", () => {
    vi.mocked(useDevices).mockReturnValue({ devices: [device], statuses: { gaming: { state: "online", pairing: "unpaired", message: "Pair this PC." } }, error: undefined, refresh: vi.fn(), act: vi.fn() });
    render(<QuickAccess/>);
    expect(screen.queryByText(/Offline/)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Pair Gaming PC" }));
    expect(navigate).toHaveBeenCalledWith("/decky-remote-power/settings");
  });
});
