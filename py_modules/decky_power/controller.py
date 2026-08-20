from __future__ import annotations

import asyncio
import time
from collections import defaultdict
from typing import Any

from .client import HostClient, HostError
from .models import DeviceState
from .store import Store, StoreError
from .wol import send_magic_packet


class Controller:
    def __init__(self, store: Store, client: HostClient | None = None):
        self.store, self.client = store, client or HostClient()
        self.states: dict[str, DeviceState] = {}; self.deadlines: dict[str, float] = {}; self.locks: defaultdict[str, asyncio.Lock] = defaultdict(asyncio.Lock)

    async def statuses(self) -> dict[str, dict[str, str]]:
        devices = self.store.load().devices
        results = await asyncio.gather(*(self.status(device.id) for device in devices), return_exceptions=True)
        return {device.id: result if isinstance(result, dict) else {"state": "unknown", "message": "Status check failed."} for device, result in zip(devices, results, strict=True)}

    async def status(self, device_id: str) -> dict[str, str]:
        device = self.store.find(device_id); secret = self.store.credentials().get(device_id)
        if secret is None: return self._result(device_id, DeviceState.UNKNOWN, "Pair this PC in Settings.")
        try: await self.client.status(device, secret)
        except HostError as error:
            if error.kind == "authentication": return self._result(device_id, DeviceState.AUTHENTICATION_FAILED, "Pairing with this PC is no longer valid.")
            if error.kind == "protocol": return self._result(device_id, DeviceState.UPDATE_REQUIRED, str(error))
            current = self.states.get(device_id)
            if current in (DeviceState.STARTING, DeviceState.STOPPING) and time.monotonic() < self.deadlines.get(device_id, 0): return self._result(device_id, current, str(error))
            return self._result(device_id, DeviceState.OFFLINE, str(error))
        return self._result(device_id, DeviceState.ONLINE, "Online")

    async def start(self, device_id: str) -> dict[str, str]:
        async with self.locks[device_id]:
            device = self.store.find(device_id); send_magic_packet(device.mac, device.broadcast_address)
            self.states[device_id], self.deadlines[device_id] = DeviceState.STARTING, time.monotonic() + 120
            return self._result(device_id, DeviceState.STARTING, "Wake-on-LAN packet sent.")

    async def stop(self, device_id: str) -> dict[str, str]:
        async with self.locks[device_id]:
            device = self.store.find(device_id); secret = self.store.credentials().get(device_id)
            if secret is None: raise StoreError("Pair this PC before shutting it down.")
            await self.client.shutdown(device, secret)
            self.states[device_id], self.deadlines[device_id] = DeviceState.STOPPING, time.monotonic() + 120
            return self._result(device_id, DeviceState.STOPPING, "Shutdown accepted.")

    def _result(self, device_id: str, state: DeviceState, message: str) -> dict[str, str]:
        self.states[device_id] = state; return {"state": state.value, "message": message}
