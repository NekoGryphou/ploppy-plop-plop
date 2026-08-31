import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { deleteDevice, getDevices } from "../api/backend";
import type { Device } from "../types";
import { Settings } from "./Settings";

const showModal = vi.fn();

vi.mock("@decky/ui", () => ({
  ButtonItem: (props: React.ButtonHTMLAttributes<HTMLButtonElement>) => <button {...props}/>,
  ConfirmModal: () => <div>Confirm deletion</div>,
  DialogButton: (props: React.ButtonHTMLAttributes<HTMLButtonElement>) => <button {...props}/>,
  Focusable: (props: React.HTMLAttributes<HTMLDivElement>) => <div {...props}/>,
  Navigation: { Navigate: vi.fn() },
  PanelSection: ({ title, children }: React.PropsWithChildren<{ title?: string }>) => <section aria-label={title}>{children}</section>,
  PanelSectionRow: ({ children }: React.PropsWithChildren) => <div>{children}</div>,
  showModal: (...arguments_: unknown[]) => showModal(...arguments_),
}));
vi.mock("../api/backend", () => ({ getDevices: vi.fn(), deleteDevice: vi.fn() }));
vi.mock("../components/DeviceForm", () => ({ DeviceForm: () => <div>Configuration form</div> }));
vi.mock("../components/PairDeviceForm", () => ({ PairDeviceForm: ({ device }: { device: Device }) => <div>Pairing form for {device.name}</div> }));

const unpaired: Device = {
  id: "gaming",
  name: "Gaming PC",
  address: "gaming-pc.local",
  port: 47991,
  mac: "AA:BB:CC:DD:EE:FF",
  mac_overridden: true,
  paired: false,
};

describe("Settings", () => {
  beforeEach(() => {
    showModal.mockReset();
    vi.mocked(getDevices).mockReset();
    vi.mocked(deleteDevice).mockReset();
  });

  it("invites configuration before pairing when no PC exists", async () => {
    const user = userEvent.setup();
    vi.mocked(getDevices).mockResolvedValue({ schemaVersion: 2, devices: [] });
    render(<Settings/>);
    expect(await screen.findByText("No PCs configured. You can add a powered-off PC now and pair it later.")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Add PC" }));
    expect(screen.getByText("Configuration form")).toBeVisible();
  });

  it("keeps an unpaired PC visible and opens a separate pairing flow", async () => {
    vi.mocked(getDevices).mockResolvedValue({ schemaVersion: 2, devices: [unpaired] });
    render(<Settings/>);
    expect(await screen.findByText("Gaming PC")).toBeVisible();
    expect(screen.getByText(/gaming-pc\.local:47991.*Not paired/)).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Pair Gaming PC" }));
    expect(screen.getByText("Pairing form for Gaming PC")).toBeVisible();
  });

  it("uses a confirmation dialog before deletion", async () => {
    vi.mocked(getDevices).mockResolvedValue({ schemaVersion: 2, devices: [unpaired] });
    render(<Settings/>);
    await screen.findByText("Gaming PC");
    fireEvent.click(screen.getByRole("button", { name: "Delete Gaming PC" }));
    await waitFor(() => expect(showModal).toHaveBeenCalledOnce());
    expect(deleteDevice).not.toHaveBeenCalled();
  });

  it("deletes only after confirmation and reloads the list", async () => {
    vi.mocked(getDevices)
      .mockResolvedValueOnce({ schemaVersion: 2, devices: [unpaired] })
      .mockResolvedValueOnce({ schemaVersion: 2, devices: [] });
    vi.mocked(deleteDevice).mockResolvedValue({ ok: true });
    render(<Settings/>);
    await screen.findByText("Gaming PC");
    fireEvent.click(screen.getByRole("button", { name: "Delete Gaming PC" }));
    const modal = showModal.mock.calls[0][0] as React.ReactElement<{ onOK: () => void }>;
    modal.props.onOK();
    await waitFor(() => expect(deleteDevice).toHaveBeenCalledWith("gaming"));
    await waitFor(() => expect(getDevices).toHaveBeenCalledTimes(2));
  });

  it("labels a configured credential as paired", async () => {
    vi.mocked(getDevices).mockResolvedValue({ schemaVersion: 2, devices: [{ ...unpaired, paired: true, host_version: "0.1.0" }] });
    render(<Settings/>);
    expect(await screen.findByText(/Paired.*Host v0\.1\.0/)).toBeVisible();
  });

  it("shows a persistent error when settings storage cannot be loaded", async () => {
    vi.mocked(getDevices).mockRejectedValue(new Error("storage unavailable"));
    render(<Settings/>);
    expect(await screen.findByRole("alert")).toHaveTextContent("settings could not be loaded");
  });
});
