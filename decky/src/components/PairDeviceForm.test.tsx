import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { pairDevice } from "../api/backend";
import type { Device } from "../types";
import { PairDeviceForm } from "./PairDeviceForm";

vi.mock("@decky/ui", () => ({
  DialogButton: (props: React.ButtonHTMLAttributes<HTMLButtonElement>) => <button {...props}/>,
  Focusable: (props: React.HTMLAttributes<HTMLDivElement>) => <div {...props}/>,
  Field: ({ label, children }: React.PropsWithChildren<{ label: string }>) => <label>{label}{children}</label>,
  TextField: (props: React.InputHTMLAttributes<HTMLInputElement>) => <input {...props}/>,
}));
vi.mock("../api/backend", () => ({ pairDevice: vi.fn() }));

const device: Device = { id: "one", name: "Gaming PC", address: "gaming.local", mac: "AA:BB:CC:DD:EE:FF", mac_overridden: true, port: 47991, paired: false };

describe("PairDeviceForm", () => {
  beforeEach(() => vi.mocked(pairDevice).mockReset());

  it("pairs the existing device and accepts the displayed spaced format", async () => {
    const onPaired = vi.fn();
    vi.mocked(pairDevice).mockResolvedValue({ ok: true, device: { ...device, paired: true } });
    render(<PairDeviceForm device={device} onPaired={onPaired} onCancel={vi.fn()}/>);
    fireEvent.change(screen.getByLabelText("Pairing code"), { target: { value: "483 921" } });
    fireEvent.click(screen.getByRole("button", { name: "Pair" }));
    await waitFor(() => expect(pairDevice).toHaveBeenCalledWith("one", "483921"));
    expect(onPaired).toHaveBeenCalledOnce();
  });

  it("keeps the existing device visible when pairing fails", async () => {
    vi.mocked(pairDevice).mockResolvedValue({ ok: false, message: "The pairing code expired." });
    render(<PairDeviceForm device={device} onPaired={vi.fn()} onCancel={vi.fn()}/>);
    fireEvent.change(screen.getByLabelText("Pairing code"), { target: { value: "483921" } });
    fireEvent.click(screen.getByRole("button", { name: "Pair" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("expired");
    expect(screen.getByText(/Gaming PC/)).toBeInTheDocument();
  });
});
