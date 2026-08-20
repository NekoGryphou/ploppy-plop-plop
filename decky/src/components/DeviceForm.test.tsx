import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DeviceForm } from "./DeviceForm";
import { discoverMac, saveDevice } from "../api/backend";

vi.mock("@decky/ui", () => ({
  DialogButton: (props: React.ButtonHTMLAttributes<HTMLButtonElement>) => <button {...props}/>,
  Focusable: (props: React.HTMLAttributes<HTMLDivElement>) => <div {...props}/>,
  Field: ({ label, description, children }: React.PropsWithChildren<{ label: string; description?: React.ReactNode }>) => <label>{label}{description}{children}</label>,
  TextField: (props: React.InputHTMLAttributes<HTMLInputElement>) => <input {...props}/>,
  Toggle: ({ value, onChange }: { value: boolean; onChange?: (value: boolean) => void }) => <input aria-label="Manual MAC" type="checkbox" checked={value} onChange={(event) => onChange?.(event.target.checked)}/>
}));
vi.mock("../api/backend", () => ({ saveDevice: vi.fn(), pairDevice: vi.fn(), discoverMac: vi.fn() }));

describe("DeviceForm", () => {
  beforeEach(() => { vi.mocked(saveDevice).mockReset(); vi.mocked(discoverMac).mockReset(); });

  it("defaults the host port and rejects malformed ports", () => {
    render(<DeviceForm onSaved={vi.fn()} onCancel={vi.fn()}/>);
    const port = screen.getByLabelText("Host port");
    expect(port).toHaveValue("47991");
    fireEvent.change(port, { target: { value: "70000" } });
    fireEvent.click(screen.getByRole("button", { name: "Save and pair" }));
    expect(screen.getByRole("alert")).toHaveTextContent("1 to 65535");
    expect(saveDevice).not.toHaveBeenCalled();
  });

  it("requires a six-digit pairing code for a new PC", () => {
    render(<DeviceForm onSaved={vi.fn()} onCancel={vi.fn()}/>);
    fireEvent.click(screen.getByRole("button", { name: "Save and pair" }));
    expect(screen.getByRole("alert")).toHaveTextContent("six-digit code");
    expect(saveDevice).not.toHaveBeenCalled();
  });

  it("detects a MAC from the address and still allows manual override", async () => {
    vi.mocked(discoverMac).mockResolvedValue({ ok: true, mac: "AA:BB:CC:DD:EE:FF", message: "Detected." });
    render(<DeviceForm onSaved={vi.fn()} onCancel={vi.fn()}/>);
    fireEvent.change(screen.getByLabelText("Address"), { target: { value: "gaming.local" } });
    fireEvent.click(screen.getByRole("button", { name: "Detect MAC" }));
    expect(await screen.findByText("AA:BB:CC:DD:EE:FF")).toBeInTheDocument();
    expect(discoverMac).toHaveBeenCalledWith("gaming.local", "47991");
    fireEvent.click(screen.getByLabelText("Manual MAC"));
    expect(screen.getByDisplayValue("AA:BB:CC:DD:EE:FF")).toBeInTheDocument();
  });
});
