import { callable } from "@decky/api";
import type { DeviceInput, DeviceSettings, MacDiscoveryResult, MutationResult, StatusResult } from "../types";

export const getDevices = callable<[], DeviceSettings>("get_devices");
export const saveDevice = callable<[values: DeviceInput, deviceId?: string], MutationResult>("save_device");
export const pairDevice = callable<[deviceId: string, pairingCode: string], MutationResult>("pair_device");
export const deleteDevice = callable<[deviceId: string], MutationResult>("delete_device");
export const getStatuses = callable<[], Record<string, StatusResult>>("get_statuses");
export const startDevice = callable<[deviceId: string], StatusResult>("start_device");
export const stopDevice = callable<[deviceId: string], StatusResult>("stop_device");
export const discoverMac = callable<[address: string, port: string], MacDiscoveryResult>("discover_mac");
