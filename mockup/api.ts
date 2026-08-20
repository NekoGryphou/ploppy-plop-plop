import { mockDevices } from "./fixtures";

export function callable(name: string) {
  return async (...arguments_: unknown[]) => {
    void arguments_;
    if (name === "discover_mac") return { ok: true, mac: "AA:BB:CC:DD:EE:FF", message: "Detected AA:BB:CC:DD:EE:FF." };
    return { schemaVersion: 2, devices: mockDevices, ok: true };
  };
}
export function definePlugin(factory: () => unknown): unknown { return factory(); }
export const routerHook = { addRoute: () => undefined, removeRoute: () => undefined };
