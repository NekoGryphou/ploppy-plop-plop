from typing import Any

import decky

from decky_power.client import HostClient, HostError
from decky_power.controller import Controller
from decky_power.discovery import DiscoveryError, discover_mac
from decky_power.store import Store, StoreError
from decky_power.validation import ValidationError


class Plugin:
    async def _main(self) -> None:
        self.store = Store(decky.DECKY_PLUGIN_SETTINGS_DIR)
        self.controller = Controller(self.store)
        decky.logger.info("Remote PC Power loaded")

    async def _unload(self) -> None: decky.logger.info("Remote PC Power unloaded")

    async def get_devices(self) -> dict[str, Any]: return self.store.load().public_dict()

    async def save_device(self, values: dict[str, Any], device_id: str | None = None) -> dict[str, Any]:
        try: device = self.store.upsert(values, device_id); return {"ok": True, "device": device.public_dict()}
        except (StoreError, ValidationError, OSError) as error: return {"ok": False, "message": str(error)}

    async def pair_device(self, device_id: str, pairing_code: str) -> dict[str, Any]:
        try:
            device = self.store.find(device_id); credential, response = await HostClient().pair(device, pairing_code)
            device = self.store.mark_paired(device_id, credential, response.host_id, response.hostname, response.host_version, response.protocol_version)
            decky.logger.info(f"Pairing succeeded for device {device_id}")
            return {"ok": True, "device": device.public_dict()}
        except (StoreError, HostError, ValueError) as error:
            decky.logger.warning(f"Pairing rejected for device {device_id}: {type(error).__name__}")
            return {"ok": False, "message": str(error)}

    async def delete_device(self, device_id: str) -> dict[str, Any]:
        try: self.store.delete(device_id); return {"ok": True}
        except (StoreError, OSError) as error: return {"ok": False, "message": str(error)}

    async def discover_mac(self, address: str, port: object) -> dict[str, Any]:
        try:
            mac = await discover_mac(address, port)
            return {"ok": True, "mac": mac, "message": f"Detected {mac}."}
        except (DiscoveryError, ValidationError) as error:
            return {"ok": False, "mac": "", "message": str(error)}

    async def get_statuses(self) -> dict[str, dict[str, str]]: return await self.controller.statuses()
    async def start_device(self, device_id: str) -> dict[str, str]: return await self.controller.start(device_id)
    async def stop_device(self, device_id: str) -> dict[str, str]: return await self.controller.stop(device_id)
