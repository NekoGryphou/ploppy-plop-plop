/**
* @export
* @desc A configured Windows PC without backend-only credential material.
*
* @property id - Stable generated device identity.
* @property name - User-facing device name.
* @property address - Hostname or IP address.
* @property mac - Normalized Wake-on-LAN address.
* @property mac_overridden - Whether the user supplied the MAC manually.
* @property port - Per-device DeckyMyRigHost port.
* @property broadcast_address - Optional IPv4 Wake-on-LAN broadcast address.
* @property host_id - Optional paired host identity.
* @property host_version - Optional reported host version.
* @property protocol_version - Optional reported protocol version.
* @property paired - Whether an authentication credential exists for this device.
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

/**
* @export
* @desc User-visible reachability/action state of one configured PC.
*/
export type DeviceState = "offline" | "starting" | "online" | "stopping" | "unknown";

/**
* @export
* @desc Authentication relationship with one configured PC, independent of reachability.
*/
export type PairingState = "unpaired" | "pairing" | "paired" | "pairing_failed" | "pairing_expired";

/**
* @export
* @desc State result returned by the Decky backend.
*
* @property state - Current device state.
* @property pairing - Current pairing state, independent of power/reachability.
* @property message - User-facing context for the state.
*/
export interface StatusResult { state: DeviceState; pairing: PairingState; message: string; }
/**
* @export
* @desc Versioned public device settings.
*
* @property schemaVersion - Persisted settings schema version.
* @property devices - Configured PCs, including unpaired PCs.
*/
export interface DeviceSettings { schemaVersion: number; devices: Device[]; }
/**
* @export
* @desc Common mutation result.
*
* @property ok - Whether the operation succeeded.
* @property message - Optional user-facing failure context.
* @property device - Optional updated device.
*/
export interface MutationResult { ok: boolean; message?: string; device?: Device; }
/**
* @export
* @desc Result of resolving a MAC from the SteamOS neighbor table.
*
* @property ok - Whether detection succeeded.
* @property message - User-facing result context.
* @property mac - Normalized detected MAC or an empty string.
*/
export interface MacDiscoveryResult { ok: boolean; message: string; mac: string; }
/**
* @export
* @desc Values collected by Add/Edit PC.
*
* @property name - User-facing PC name.
* @property address - Hostname or IP address.
* @property mac - Manually entered or optionally detected MAC.
* @property macOverridden - Whether manual MAC behavior is authoritative.
* @property port - User-entered per-PC host port.
* @property broadcastAddress - Optional IPv4 broadcast address.
*/
export interface DeviceInput { name: string; address: string; mac: string; macOverridden: boolean; port: string; broadcastAddress: string; }
