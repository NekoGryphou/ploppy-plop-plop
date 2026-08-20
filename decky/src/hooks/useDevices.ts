import { useCallback, useEffect, useRef, useState } from "react";
import { getDevices, getStatuses, startDevice, stopDevice } from "../api/backend";
import type { Device, StatusResult } from "../types";

/**
* @public
* @desc Loads devices and polls each device without leaving runaway timers.
*
* @returns Device data and state actions for the Quick Access panel.
*/
export function useDevices() {
  const [devices, setDevices] = useState<Device[]>([]);
  const [statuses, setStatuses] = useState<Record<string, StatusResult>>({});
  const [error, setError] = useState<string>();
  const mounted = useRef(true);

  const refresh = useCallback(async (): Promise<void> => {
    try {
      const [settings, nextStatuses] = await Promise.all([getDevices(), getStatuses()]);
      if (mounted.current) { setDevices(settings.devices); setStatuses(nextStatuses); setError(undefined); }
    } catch { if (mounted.current) setError("PC status could not be refreshed."); }
  }, []);

  useEffect(() => {
    mounted.current = true;
    void refresh();
    return () => { mounted.current = false; };
  }, [refresh]);

  useEffect(() => {
    const pending = Object.values(statuses).some(({ state }) => state === "starting" || state === "stopping");
    const timer = window.setInterval(() => { void refresh(); }, pending ? 2_000 : 15_000);
    return () => { window.clearInterval(timer); };
  }, [refresh, statuses]);

  const act = useCallback(async (device: Device): Promise<void> => {
    const current = statuses[device.id]?.state;
    if (current === "starting" || current === "stopping") return;
    setStatuses((value) => ({ ...value, [device.id]: { state: current === "online" ? "stopping" : "starting", message: "Request in progress." } }));
    try {
      const result = current === "online" ? await stopDevice(device.id) : await startDevice(device.id);
      setStatuses((value) => ({ ...value, [device.id]: result }));
      window.setTimeout(() => { void refresh(); }, 2_000);
    } catch { setStatuses((value) => ({ ...value, [device.id]: { state: "unknown", message: "The action failed. Try again." } })); }
  }, [refresh, statuses]);

  return { devices, statuses, error, refresh, act };
}
