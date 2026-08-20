import asyncio
import tempfile
import unittest
from unittest.mock import patch

from decky_power.client import HostError
from decky_power.controller import Controller
from decky_power.models import DeviceState
from decky_power.store import Store


class FakeClient:
    def __init__(self) -> None:
        self.status_ports: list[int] = []
        self.shutdown_ports: list[int] = []
        self.unavailable: set[int] = set()

    async def status(self, device, credential):
        del credential
        self.status_ports.append(device.port)
        await asyncio.sleep(0)
        if device.port in self.unavailable: raise HostError("unavailable", "Host unavailable")
        return object()

    async def shutdown(self, device, credential) -> None:
        del credential
        self.shutdown_ports.append(device.port)


class ControllerTests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.store = Store(self.directory.name)
        self.first = self.store.upsert({"name": "Gaming", "address": "one.local", "mac": "AABBCCDDEEFF", "port": 47991, "broadcastAddress": ""})
        self.second = self.store.upsert({"name": "Bedroom", "address": "two.local", "mac": "112233445566", "port": 48100, "broadcastAddress": ""})
        credentials = {self.first.id: b"a" * 32, self.second.id: b"b" * 32}
        self.store.save_credentials(credentials)
        self.client = FakeClient()
        self.controller = Controller(self.store, self.client)

    async def asyncTearDown(self) -> None: self.directory.cleanup()

    async def test_multiple_devices_use_independent_ports(self) -> None:
        statuses = await self.controller.statuses()
        self.assertEqual({statuses[self.first.id]["state"], statuses[self.second.id]["state"]}, {DeviceState.ONLINE.value})
        self.assertCountEqual(self.client.status_ports, [47991, 48100])
        await self.controller.stop(self.second.id)
        self.assertEqual(self.client.shutdown_ports, [48100])

    async def test_start_and_stop_transitions(self) -> None:
        with patch("decky_power.controller.send_magic_packet") as wake:
            started = await self.controller.start(self.first.id)
            wake.assert_called_once_with("AA:BB:CC:DD:EE:FF", None)
        self.assertEqual(started["state"], DeviceState.STARTING.value)
        stopped = await self.controller.stop(self.first.id)
        self.assertEqual(stopped["state"], DeviceState.STOPPING.value)

    async def test_unreachable_host_becomes_offline(self) -> None:
        self.client.unavailable.add(47991)
        result = await self.controller.status(self.first.id)
        self.assertEqual(result["state"], DeviceState.OFFLINE.value)
        self.assertEqual(result["message"], "Host unavailable")


if __name__ == "__main__": unittest.main()
