import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { getDevices, getStatuses, startDevice } from "../api/backend";
import type { Device } from "../types";
import { useDevices } from "./useDevices";

vi.mock("../api/backend", () => ({
  getDevices: vi.fn(),
  getStatuses: vi.fn(),
  startDevice: vi.fn(),
  stopDevice: vi.fn(),
}));

describe("useDevices", () => {
  beforeEach(() => {
    vi.mocked(getDevices).mockReset();
    vi.mocked(getStatuses).mockReset();
    vi.mocked(startDevice).mockReset();
  });

  it("keeps refresh single-flight while a poll is pending", async () => {
    let finishDevices!: (value: { schemaVersion: number; devices: [] }) => void;
    let finishStatuses!: (value: Record<string, never>) => void;
    vi.mocked(getDevices).mockReturnValue(new Promise((resolve) => { finishDevices = resolve; }));
    vi.mocked(getStatuses).mockReturnValue(new Promise((resolve) => { finishStatuses = resolve; }));
    const { result } = renderHook(() => useDevices());

    act(() => {
      void result.current.refresh();
      void result.current.refresh();
    });
    expect(getDevices).toHaveBeenCalledOnce();
    expect(getStatuses).toHaveBeenCalledOnce();

    finishDevices({ schemaVersion: 2, devices: [] });
    finishStatuses({});
    await waitFor(() => expect(result.current.error).toBeUndefined());
  });

  it("does not schedule action refreshes after unmount", async () => {
    const device: Device = {
      id: "one", name: "Gaming", address: "gaming.local", mac: "AA:BB:CC:DD:EE:FF",
      mac_overridden: true, port: 47991, paired: false,
    };
    let finishAction!: (value: { state: "starting"; pairing: "unpaired"; message: string }) => void;
    vi.mocked(getDevices).mockResolvedValue({ schemaVersion: 2, devices: [device] });
    vi.mocked(getStatuses).mockResolvedValue({
      one: { state: "offline", pairing: "unpaired", message: "Offline" },
    });
    vi.mocked(startDevice).mockReturnValue(new Promise((resolve) => { finishAction = resolve; }));
    const { result, unmount } = renderHook(() => useDevices());
    await waitFor(() => expect(result.current.devices).toHaveLength(1));

    let action!: Promise<void>;
    act(() => { action = result.current.act(device); });
    unmount();
    const timeout = vi.spyOn(window, "setTimeout");
    finishAction({ state: "starting", pairing: "unpaired", message: "Sent" });
    await action;

    expect(timeout).not.toHaveBeenCalled();
    timeout.mockRestore();
  });

  it("does not update action errors after unmount", async () => {
    const device: Device = {
      id: "one", name: "Gaming", address: "gaming.local", mac: "AA:BB:CC:DD:EE:FF",
      mac_overridden: true, port: 47991, paired: false,
    };
    let rejectAction!: (reason: Error) => void;
    vi.mocked(getDevices).mockResolvedValue({ schemaVersion: 2, devices: [device] });
    vi.mocked(getStatuses).mockResolvedValue({
      one: { state: "offline", pairing: "unpaired", message: "Offline" },
    });
    vi.mocked(startDevice).mockReturnValue(new Promise((_, reject) => { rejectAction = reject; }));
    const { result, unmount } = renderHook(() => useDevices());
    await waitFor(() => expect(result.current.devices).toHaveLength(1));

    let action!: Promise<void>;
    act(() => { action = result.current.act(device); });
    unmount();
    const timeout = vi.spyOn(window, "setTimeout");
    rejectAction(new Error("injected failure"));
    await action;

    expect(timeout).not.toHaveBeenCalled();
    timeout.mockRestore();
  });
});
