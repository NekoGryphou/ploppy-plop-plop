import type { Device } from "../src/types";

export const mockDevices: Device[] = [
  { id: "gaming", name: "Gaming PC", address: "gaming-pc.local", mac: "AA:BB:CC:DD:EE:FF", mac_overridden: false, port: 47991, paired: true, host_version: "1.0.0" },
  { id: "bedroom", name: "Bedroom PC", address: "192.168.1.42", mac: "11:22:33:44:55:66", mac_overridden: true, port: 48100, paired: true, host_version: "1.0.0" },
  { id: "workstation", name: "Workstation", address: "workstation.local", mac: "22:33:44:55:66:77", mac_overridden: false, port: 47991, paired: false }
];
