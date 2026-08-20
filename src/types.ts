/**
* @export
* @desc A configured Windows PC without backend-only credential material.
*
* @property id - Stable generated device identity.
* @property name - User-facing device name.
* @property address - Hostname or IP address.
* @property mac - Normalized Wake-on-LAN address.
* @property port - Per-device DeckyPowerHost port.
*/
export interface Device {
  /** Stable UUID. */ id: string;
  /** Display name. */ name: string;
  /** Hostname or IP. */ address: string;
  /** Normalized MAC. */ mac: string;
  /** Whether the user chose a manual MAC instead of discovery. */ mac_overridden: boolean;
  /** Host TCP port. */ port: number;
  /** Optional WOL broadcast IPv4 address. */ broadcast_address?: string | null;
  /** Paired host UUID. */ host_id?: string | null;
  /** Reported host version. */ host_version?: string | null;
  /** Reported protocol version. */ protocol_version?: number | null;
  /** Whether pairing completed. */ paired: boolean;
}

/** @export @desc User-visible state of one configured PC. */
export type DeviceState = "offline" | "starting" | "online" | "stopping" | "unknown" | "authentication_failed" | "host_unavailable" | "update_required";

/** @export @desc State result returned by the Decky backend. */
export interface StatusResult { state: DeviceState; message: string; }
/** @export @desc Versioned public device settings. */
export interface DeviceSettings { schemaVersion: number; devices: Device[]; }
/** @export @desc Common mutation result. */
export interface MutationResult { ok: boolean; message?: string; device?: Device; }
/** @export @desc Result of resolving a MAC from the SteamOS neighbor table. */
export interface MacDiscoveryResult { ok: boolean; message: string; mac: string; }
/** @export @desc Values collected by Add/Edit PC. */
export interface DeviceInput { name: string; address: string; mac: string; macOverridden: boolean; port: string; broadcastAddress: string; }
