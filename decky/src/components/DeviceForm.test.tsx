import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DeviceForm } from "./DeviceForm";
import { discoverMac, pairDevice, saveDevice } from "../api/backend";

vi.mock("@decky/ui", () => ({
  DialogButton: (props: React.ButtonHTMLAttributes<HTMLButtonElement>) => <button {...props}/>,
  Focusable: (props: React.HTMLAttributes<HTMLDivElement>) => <div {...props}/>,
  Field: ({ label, description, children }: React.PropsWithChildren<{ label: string; description?: React.ReactNode }>) => <label>{label}{description}{children}</label>,
  TextField: (props: React.InputHTMLAttributes<HTMLInputElement>) => <input {...props}/>,
}));
vi.mock("../api/backend", () => ({ saveDevice: vi.fn(), pairDevice: vi.fn(), discoverMac: vi.fn() }));

describe("DeviceForm", () => {
  beforeEach(() => {
    vi.mocked(saveDevice).mockReset();
    vi.mocked(pairDevice).mockReset();
    vi.mocked(discoverMac).mockReset();
  });

  it("defaults the host port and rejects malformed ports", () => {
    render(<DeviceForm onSaved={vi.fn()} onCancel={vi.fn()}/>);
    const port = screen.getByLabelText("Host port");
    expect(port).toHaveValue("47991");
    fireEvent.change(port, { target: { value: "70000" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(screen.getByRole("alert")).toHaveTextContent("1 to 65535");
    expect(saveDevice).not.toHaveBeenCalled();
  });

  it("saves a manually configured PC without pairing or network access", async () => {
    const onSaved = vi.fn();
    vi.mocked(saveDevice).mockResolvedValue({
      ok: true,
      device: {
        id: "gaming",
        name: "Gaming PC",
        address: "gaming.local",
        mac: "AA:BB:CC:DD:EE:FF",
        mac_overridden: true,
        port: 47991,
        paired: false,
      },
    });
    render(<DeviceForm onSaved={onSaved} onCancel={vi.fn()}/>);
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Gaming PC" } });
    fireEvent.change(screen.getByLabelText("Address"), { target: { value: "gaming.local" } });
    fireEvent.change(screen.getByLabelText("MAC address"), { target: { value: "AA:BB:CC:DD:EE:FF" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    expect(await screen.findByRole("button", { name: "Save" })).toBeEnabled();
    expect(saveDevice).toHaveBeenCalledWith({
      name: "Gaming PC",
      address: "gaming.local",
      mac: "AA:BB:CC:DD:EE:FF",
      macOverridden: true,
      port: "47991",
      broadcastAddress: "",
    }, undefined);
    expect(discoverMac).not.toHaveBeenCalled();
    expect(pairDevice).not.toHaveBeenCalled();
    expect(onSaved).toHaveBeenCalledOnce();
  });

  it("offers MAC detection without hiding manual entry", async () => {
    vi.mocked(discoverMac).mockResolvedValue({ ok: true, mac: "AA:BB:CC:DD:EE:FF", message: "Detected." });
    render(<DeviceForm onSaved={vi.fn()} onCancel={vi.fn()}/>);
    fireEvent.change(screen.getByLabelText("Address"), { target: { value: "gaming.local" } });
    fireEvent.click(screen.getByRole("button", { name: "Detect MAC" }));
    expect(await screen.findByDisplayValue("AA:BB:CC:DD:EE:FF")).toBeInTheDocument();
    expect(discoverMac).toHaveBeenCalledWith("gaming.local", "47991");
  });

  it("edits the existing PC without pairing or changing its identity", async () => {
    const device = {
      id: "gaming",
      name: "Gaming PC",
      address: "gaming.local",
      mac: "AA:BB:CC:DD:EE:FF",
      mac_overridden: true,
      port: 47991,
      paired: false,
    };
    vi.mocked(saveDevice).mockResolvedValue({ ok: true, device: { ...device, name: "Main Gaming PC", port: 48100 } });
    render(<DeviceForm device={device} onSaved={vi.fn()} onCancel={vi.fn()}/>);
    fireEvent.change(screen.getByLabelText("Name"), { target: { value: "Main Gaming PC" } });
    fireEvent.change(screen.getByLabelText("Host port"), { target: { value: "48100" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(saveDevice).toHaveBeenCalledWith(expect.objectContaining({
      name: "Main Gaming PC",
      port: "48100",
    }), "gaming"));
    expect(pairDevice).not.toHaveBeenCalled();
  });

  it("keeps manual MAC entry usable when automatic detection fails", async () => {
    vi.mocked(discoverMac).mockRejectedValue(new Error("neighbor lookup failed"));
    render(<DeviceForm onSaved={vi.fn()} onCancel={vi.fn()}/>);
    fireEvent.change(screen.getByLabelText("Address"), { target: { value: "gaming.local" } });
    fireEvent.click(screen.getByRole("button", { name: "Detect MAC" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Enter it manually");
    expect(screen.getByLabelText("MAC address")).toBeEnabled();
  });

  it("shows a persistent error when saving fails unexpectedly", async () => {
    vi.mocked(saveDevice).mockRejectedValue(new Error("backend unavailable"));
    render(<DeviceForm onSaved={vi.fn()} onCancel={vi.fn()}/>);
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("existing configuration was not changed");
    expect(screen.getByRole("button", { name: "Save" })).toBeEnabled();
  });
});
