from __future__ import annotations

import asyncio
import logging
import time
from collections import defaultdict

from .client import HostClient, HostError
from .models import DeviceState, PairingState
from .store import Store, StoreError
from .wol import send_magic_packet
from .protobuf import PLUGIN_VERSION
from .versioning import VersionRelation, compare_versions

logger = logging.getLogger(__name__)


class Controller:
    def __init__(
        self,
        store: Store,
        client: HostClient | None = None,
        *,
        wol_ports: tuple[int, ...] | None = None,
        transition_timeout: float = 120,
        status_concurrency: int = 8,
    ):
        self.store = store
        self.client = client or HostClient()
        self.wol_ports = wol_ports
        self.transition_timeout = transition_timeout
        if status_concurrency < 1:
            raise ValueError("status_concurrency must be at least one")
        self.status_concurrency = status_concurrency
        self.states: dict[str, DeviceState] = {}
        self.deadlines: dict[str, float] = {}
        self.locks: defaultdict[str, asyncio.Lock] = defaultdict(asyncio.Lock)

    async def statuses(self) -> dict[str, dict[str, str]]:
        settings, credentials = self.store.snapshot()
        semaphore = asyncio.Semaphore(self.status_concurrency)

        async def limited_status(device):
            async with semaphore:
                return await self._status(device, credentials.get(device.id))

        results = await asyncio.gather(
            *(limited_status(device) for device in settings.devices),
            return_exceptions=True,
        )
        for device, result in zip(settings.devices, results, strict=True):
            if isinstance(result, BaseException):
                logger.error(
                    "Unexpected status failure for device %s",
                    device.id,
                    exc_info=(type(result), result, result.__traceback__),
                )
        return {
            device.id: (
                result
                if isinstance(result, dict)
                else {
                    "state": DeviceState.UNKNOWN.value,
                    "pairing": PairingState.PAIRED.value
                    if device.paired
                    else PairingState.UNPAIRED.value,
                    "message": "Status check failed.",
                }
            )
            for device, result in zip(settings.devices, results, strict=True)
        }

    async def status(self, device_id: str) -> dict[str, str]:
        device = self.store.find(device_id)
        return await self._status(device, self.store.credentials().get(device_id))

    async def _status(self, device, secret: bytes | None) -> dict[str, str]:
        device_id = device.id
        if secret is None:
            try:
                await self.client.probe(device)
            except HostError as error:
                if error.kind == "protocol":
                    return self._result(
                        device_id, DeviceState.UNKNOWN, PairingState.UNPAIRED, str(error)
                    )
                if (
                    self.states.get(device_id) == DeviceState.STARTING
                    and time.monotonic() < self.deadlines.get(device_id, 0)
                ):
                    return self._result(
                        device_id, DeviceState.STARTING, PairingState.UNPAIRED, str(error)
                    )
                return self._result(device_id, DeviceState.OFFLINE, PairingState.UNPAIRED, str(error))
            return self._result(
                device_id,
                DeviceState.ONLINE,
                PairingState.UNPAIRED,
                "DeckyPowerHost was found. Pair this PC in Settings.",
            )
        try:
            response = await self.client.status(device, secret)
        except HostError as error:
            if error.kind == "authentication":
                return self._result(
                    device_id,
                    DeviceState.ONLINE,
                    PairingState.PAIRING_FAILED,
                    "Pairing with this PC is no longer valid.",
                )
            if error.kind == "integrity":
                return self._result(device_id, DeviceState.UNKNOWN, PairingState.PAIRED, str(error))
            if error.kind == "protocol":
                return self._result(device_id, DeviceState.UNKNOWN, PairingState.PAIRED, str(error))
            if error.kind == "identity":
                return self._result(
                    device_id, DeviceState.UNKNOWN, PairingState.PAIRING_FAILED, str(error)
                )
            current = self.states.get(device_id)
            if current in (
                DeviceState.STARTING,
                DeviceState.STOPPING,
            ) and time.monotonic() < self.deadlines.get(device_id, 0):
                return self._result(device_id, current, PairingState.PAIRED, str(error))
            return self._result(device_id, DeviceState.OFFLINE, PairingState.PAIRED, str(error))
        relation = compare_versions(PLUGIN_VERSION, response.host_version)
        message = {
            VersionRelation.UPDATE_HOST: f"Update DeckyPowerHost (host {response.host_version}, plugin {PLUGIN_VERSION}).",
            VersionRelation.UPDATE_PLUGIN: f"Update the Decky plugin (plugin {PLUGIN_VERSION}, host {response.host_version}).",
            VersionRelation.INCOMPATIBLE: f"Host and plugin major versions are incompatible ({response.host_version} vs {PLUGIN_VERSION}).",
            VersionRelation.UNKNOWN: f"Version compatibility is unknown (host reported {response.host_version!r}).",
        }.get(relation, "Online")
        return self._result(device_id, DeviceState.ONLINE, PairingState.PAIRED, message)

    async def start(self, device_id: str) -> dict[str, str]:
        async with self.locks[device_id]:
            device = self.store.find(device_id)
            if self.wol_ports is None:
                send_magic_packet(device.mac, device.broadcast_address)
            else:
                send_magic_packet(device.mac, device.broadcast_address, self.wol_ports)
            self.states[device_id] = DeviceState.STARTING
            self.deadlines[device_id] = time.monotonic() + self.transition_timeout
            pairing = (
                PairingState.PAIRED
                if device.id in self.store.credentials()
                else PairingState.UNPAIRED
            )
            return self._result(device_id, DeviceState.STARTING, pairing, "Wake-on-LAN packet sent.")

    async def stop(self, device_id: str) -> dict[str, str]:
        async with self.locks[device_id]:
            device = self.store.find(device_id)
            secret = self.store.credentials().get(device_id)
            if secret is None:
                raise StoreError("Pair this PC before shutting it down.")
            await self.client.shutdown(device, secret)
            self.states[device_id] = DeviceState.STOPPING
            self.deadlines[device_id] = time.monotonic() + self.transition_timeout
            return self._result(device_id, DeviceState.STOPPING, PairingState.PAIRED, "Shutdown accepted.")

    def _result(
        self, device_id: str, state: DeviceState, pairing: PairingState, message: str
    ) -> dict[str, str]:
        self.states[device_id] = state
        return {"state": state.value, "pairing": pairing.value, "message": message}
