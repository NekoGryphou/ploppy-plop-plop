from typing import Any

import decky

from decky_power.client import HostClient, HostError
from decky_power.controller import Controller
from decky_power.discovery import DiscoveryError, discover_mac
from decky_power.store import Store, StoreError
from decky_power.validation import ValidationError


class Plugin:
    async def _main(self) -> None:
        try:
            self.store = Store(decky.DECKY_PLUGIN_SETTINGS_DIR)
            self.controller = Controller(self.store)
        except Exception:
            decky.logger.exception("Remote PC Power failed to initialize")
            raise
        decky.logger.info("Remote PC Power loaded")

    async def _unload(self) -> None:
        decky.logger.info("Remote PC Power unloaded")

    async def get_devices(self) -> dict[str, Any]:
        try:
            return self.store.load().public_dict()
        except Exception:
            decky.logger.exception("Could not load device settings")
            raise

    async def save_device(self, values: dict[str, Any], device_id: str | None = None) -> dict[str, Any]:
        try:
            device = self.store.upsert(values, device_id)
            return {"ok": True, "device": device.public_dict()}
        except (StoreError, ValidationError) as error:
            decky.logger.warning(
                "Device configuration rejected for %s: %s",
                device_id or "new device",
                type(error).__name__,
            )
            return {"ok": False, "message": str(error)}
        except OSError:
            decky.logger.exception("Could not persist device %s", device_id or "new device")
            return {"ok": False, "message": "The PC configuration could not be saved."}
        except Exception:
            decky.logger.exception("Unexpected failure while saving device %s", device_id or "new device")
            return {"ok": False, "message": "An unexpected error prevented the PC from being saved."}

    async def pair_device(self, device_id: str, pairing_code: str) -> dict[str, Any]:
        try:
            device = self.store.find(device_id); credential, response = await HostClient().pair(device, pairing_code)
            device = self.store.mark_paired(device_id, credential, response.host_id, response.hostname, response.host_version, response.protocol_version)
            decky.logger.info(f"Pairing succeeded for device {device_id}")
            return {"ok": True, "device": device.public_dict()}
        except (StoreError, HostError, ValueError) as error:
            decky.logger.warning(f"Pairing rejected for device {device_id}: {type(error).__name__}")
            return {"ok": False, "message": str(error)}
        except OSError:
            decky.logger.exception("Could not persist pairing for device %s", device_id)
            return {"ok": False, "message": "Pairing could not be saved on this Steam Deck. Retry pairing."}
        except Exception:
            decky.logger.exception("Unexpected pairing failure for device %s", device_id)
            return {"ok": False, "message": "An unexpected error interrupted pairing."}

    async def delete_device(self, device_id: str) -> dict[str, Any]:
        try:
            self.store.delete(device_id)
            return {"ok": True}
        except StoreError as error:
            decky.logger.warning("Device deletion rejected for %s: %s", device_id, type(error).__name__)
            return {"ok": False, "message": str(error)}
        except OSError:
            decky.logger.exception("Could not persist deletion for device %s", device_id)
            return {"ok": False, "message": "The PC could not be deleted because settings could not be saved."}
        except Exception:
            decky.logger.exception("Unexpected failure deleting device %s", device_id)
            return {"ok": False, "message": "An unexpected error prevented deletion."}

    async def discover_mac(self, address: str, port: object) -> dict[str, Any]:
        try:
            mac = await discover_mac(address, port)
            return {"ok": True, "mac": mac, "message": f"Detected {mac}."}
        except (DiscoveryError, ValidationError) as error:
            return {"ok": False, "mac": "", "message": str(error)}
        except Exception:
            decky.logger.exception("Unexpected MAC discovery failure")
            return {"ok": False, "mac": "", "message": "MAC discovery failed unexpectedly."}

    async def get_statuses(self) -> dict[str, dict[str, str]]:
        try:
            return await self.controller.statuses()
        except Exception:
            decky.logger.exception("Could not collect device statuses")
            raise

    async def start_device(self, device_id: str) -> dict[str, str]:
        try:
            return await self.controller.start(device_id)
        except Exception:
            decky.logger.exception("Could not start device %s", device_id)
            raise

    async def stop_device(self, device_id: str) -> dict[str, str]:
        try:
            return await self.controller.stop(device_id)
        except Exception:
            decky.logger.exception("Could not stop device %s", device_id)
            raise
