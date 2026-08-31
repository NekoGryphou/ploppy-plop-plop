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
  const refreshGeneration = useRef(0);
  const refreshInFlight = useRef<Promise<void> | null>(null);
  const actionTimers = useRef<Set<number>>(new Set());

  const refresh = useCallback(async (): Promise<void> => {
    if (refreshInFlight.current) return refreshInFlight.current;
    const generation = ++refreshGeneration.current;
    const request = (async (): Promise<void> => {
      try {
        const [settings, nextStatuses] = await Promise.all([getDevices(), getStatuses()]);
        if (mounted.current && generation === refreshGeneration.current) { setDevices(settings.devices); setStatuses(nextStatuses); setError(undefined); }
      } catch { if (mounted.current && generation === refreshGeneration.current) setError("PC status could not be refreshed."); }
    })();
    refreshInFlight.current = request;
    try { await request; }
    finally { if (refreshInFlight.current === request) refreshInFlight.current = null; }
  }, []);

  useEffect(() => {
    mounted.current = true;
    void refresh();
    return () => {
      mounted.current = false;
      refreshGeneration.current += 1;
      for (const timer of actionTimers.current) window.clearTimeout(timer);
      actionTimers.current.clear();
    };
  }, [refresh]);

  useEffect(() => {
    const pending = Object.values(statuses).some(({ state }) => state === "starting" || state === "stopping");
    const timer = window.setInterval(() => { void refresh(); }, pending ? 2_000 : 15_000);
    return () => { window.clearInterval(timer); };
  }, [refresh, statuses]);

  const act = useCallback(async (device: Device): Promise<void> => {
    const current = statuses[device.id]?.state;
    const pairing = statuses[device.id]?.pairing ?? (device.paired ? "paired" : "unpaired");
    if (current === "starting" || current === "stopping") return;
    setStatuses((value) => ({ ...value, [device.id]: { state: current === "online" ? "stopping" : "starting", pairing, message: "Request in progress." } }));
    try {
      const result = current === "online" ? await stopDevice(device.id) : await startDevice(device.id);
      if (!mounted.current) return;
      setStatuses((value) => ({ ...value, [device.id]: result }));
      const timer = window.setTimeout(() => {
        actionTimers.current.delete(timer);
        void refresh();
      }, 2_000);
      actionTimers.current.add(timer);
    } catch {
      if (mounted.current) setStatuses((value) => ({ ...value, [device.id]: { state: "unknown", pairing, message: "The action failed. Try again." } }));
    }
  }, [refresh, statuses]);

  return { devices, statuses, error, refresh, act };
}
