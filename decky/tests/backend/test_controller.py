import asyncio
import tempfile
import unittest
from types import SimpleNamespace
from unittest.mock import patch

from decky_power.client import HostError
from decky_power.controller import Controller
from decky_power.models import DeviceState, PairingState
from decky_power.store import Store


class FakeClient:
    def __init__(self) -> None:
        self.status_ports: list[int] = []
        self.shutdown_ports: list[int] = []
        self.unavailable: set[int] = set()
        self.delays: dict[int, float] = {}
        self.errors: dict[int, BaseException] = {}
        self.active_statuses = 0
        self.max_active_statuses = 0
        self.host_versions: dict[int, str] = {}

    async def probe(self, device) -> None:
        await asyncio.sleep(0)
        if device.port in self.unavailable: raise HostError("unavailable", "Host unavailable")

    async def status(self, device, credential):
        del credential
        self.status_ports.append(device.port)
        self.active_statuses += 1
        self.max_active_statuses = max(self.max_active_statuses, self.active_statuses)
        try:
            await asyncio.sleep(self.delays.get(device.port, 0))
            if device.port in self.errors: raise self.errors[device.port]
            if device.port in self.unavailable: raise HostError("unavailable", "Host unavailable")
            return SimpleNamespace(host_version=self.host_versions.get(device.port, "0.1.0"))
        finally:
            self.active_statuses -= 1

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
        self.store._save_credentials(credentials)
        self.client = FakeClient()
        self.controller = Controller(self.store, self.client)

    async def asyncTearDown(self) -> None: self.directory.cleanup()

    async def test_multiple_devices_use_independent_ports(self) -> None:
        with patch.object(self.store, "credentials", wraps=self.store.credentials) as credentials:
            statuses = await self.controller.statuses()
        self.assertEqual(credentials.call_count, 1)
        self.assertEqual({statuses[self.first.id]["state"], statuses[self.second.id]["state"]}, {DeviceState.ONLINE.value})
        self.assertCountEqual(self.client.status_ports, [47991, 48100])
        await self.controller.stop(self.second.id)
        self.assertEqual(self.client.shutdown_ports, [48100])

    async def test_start_and_stop_transitions(self) -> None:
        with patch("decky_power.controller.send_magic_packet") as wake:
            started = await self.controller.start(self.first.id)
            wake.assert_called_once_with("AA:BB:CC:DD:EE:FF", None)
        self.assertEqual(started["state"], DeviceState.STARTING.value)
        self.assertEqual(started["pairing"], PairingState.PAIRED.value)
        stopped = await self.controller.stop(self.first.id)
        self.assertEqual(stopped["state"], DeviceState.STOPPING.value)

    async def test_unreachable_host_becomes_offline(self) -> None:
        self.client.unavailable.add(47991)
        result = await self.controller.status(self.first.id)
        self.assertEqual(result["state"], DeviceState.OFFLINE.value)
        self.assertEqual(result["message"], "Host unavailable")

    async def test_untrusted_response_does_not_invalidate_pairing(self) -> None:
        self.client.errors[47991] = HostError("integrity", "Response could not be authenticated.")
        result = await self.controller.status(self.first.id)
        self.assertEqual(result["state"], DeviceState.UNKNOWN.value)
        self.assertEqual(result["pairing"], PairingState.PAIRED.value)

    async def test_host_identity_mismatch_requires_repairing(self) -> None:
        self.client.errors[47991] = HostError("identity", "This address now belongs to a different PC.")
        result = await self.controller.status(self.first.id)
        self.assertEqual(result["state"], DeviceState.UNKNOWN.value)
        self.assertEqual(result["pairing"], PairingState.PAIRING_FAILED.value)
        self.assertIn("different PC", result["message"])

    async def test_minor_version_difference_reports_the_update_direction(self) -> None:
        self.client.host_versions[self.first.port] = "0.0.9"
        update_host = await self.controller.status(self.first.id)
        self.assertIn("Update DeckyPowerHost", update_host["message"])

        self.client.host_versions[self.first.port] = "0.2.0"
        update_plugin = await self.controller.status(self.first.id)
        self.assertIn("Update the Decky plugin", update_plugin["message"])

        self.client.host_versions[self.first.port] = "0.1.99"
        patch_only = await self.controller.status(self.first.id)
        self.assertEqual(patch_only["message"], "Online")

    async def test_unpaired_device_can_wake_but_cannot_shutdown(self) -> None:
        unpaired = self.store.upsert({"name": "Unpaired", "address": "off.local", "mac": "AABBCCDDEEFF", "port": 49000, "broadcastAddress": "192.168.1.255"})
        with patch("decky_power.controller.send_magic_packet") as wake:
            result = await self.controller.start(unpaired.id)
        wake.assert_called_once_with("AA:BB:CC:DD:EE:FF", "192.168.1.255")
        self.assertEqual(result["state"], DeviceState.STARTING.value)
        self.assertEqual(result["pairing"], PairingState.UNPAIRED.value)
        self.client.unavailable.add(49000)
        polled = await self.controller.status(unpaired.id)
        self.assertEqual(polled["state"], DeviceState.STARTING.value)
        self.controller.deadlines[unpaired.id] = 0
        timed_out = await self.controller.status(unpaired.id)
        self.assertEqual(timed_out["state"], DeviceState.OFFLINE.value)
        with self.assertRaisesRegex(Exception, "Pair this PC"):
            await self.controller.stop(unpaired.id)

    async def test_reachable_unpaired_host_requires_pairing_without_losing_device(self) -> None:
        unpaired = self.store.upsert({"name": "Unpaired", "address": "on.local", "mac": "AABBCCDDEEFF", "port": 49000, "broadcastAddress": ""})
        result = await self.controller.status(unpaired.id)
        self.assertEqual(result["state"], DeviceState.ONLINE.value)
        self.assertEqual(result["pairing"], PairingState.UNPAIRED.value)
        self.assertEqual(self.store.find(unpaired.id).name, "Unpaired")

    async def test_slow_and_broken_hosts_do_not_block_other_results(self) -> None:
        slow = self.store.upsert({"name": "Slow", "address": "slow.local", "mac": "223344556677", "port": 48200, "broadcastAddress": ""})
        wrong_port = self.store.upsert({"name": "Wrong port", "address": "wrong.local", "mac": "334455667788", "port": 48201, "broadcastAddress": ""})
        credentials = self.store.credentials()
        credentials.update({slow.id: b"c" * 32, wrong_port.id: b"d" * 32})
        self.store._save_credentials(credentials)
        self.client.delays[48200] = 0.05
        self.client.unavailable.add(48201)

        results = await asyncio.wait_for(self.controller.statuses(), timeout=0.2)

        self.assertEqual(results[self.first.id]["state"], DeviceState.ONLINE.value)
        self.assertEqual(results[self.second.id]["state"], DeviceState.ONLINE.value)
        self.assertEqual(results[slow.id]["state"], DeviceState.ONLINE.value)
        self.assertEqual(results[wrong_port.id]["state"], DeviceState.OFFLINE.value)

    async def test_bulk_status_polling_has_an_explicit_concurrency_limit(self) -> None:
        credentials = self.store.credentials()
        for index in range(6):
            port = 49000 + index
            device = self.store.upsert({"name": f"PC {index}", "address": f"pc-{index}.local", "mac": f"AABBCCDDEE{index:02X}", "port": port, "broadcastAddress": ""})
            credentials[device.id] = bytes([index + 1]) * 32
            self.client.delays[port] = 0.01
        self.store._save_credentials(credentials)
        controller = Controller(self.store, self.client, status_concurrency=2)

        await controller.statuses()

        self.assertLessEqual(self.client.max_active_statuses, 2)

    async def test_large_status_batch_is_bounded_cancellable_and_releases_workers(self) -> None:
        credentials = self.store.credentials()
        for index in range(100):
            port = 50000 + index
            device = self.store.upsert({"name": f"Bulk {index}", "address": f"bulk-{index}.local", "mac": f"AABBCC{index:06X}", "port": port, "broadcastAddress": ""})
            credentials[device.id] = bytes([(index % 255) + 1]) * 32
            self.client.delays[port] = 60
        self.store._save_credentials(credentials)
        controller = Controller(self.store, self.client, status_concurrency=4)
        task = asyncio.create_task(controller.statuses())
        while self.client.max_active_statuses < 4:
            await asyncio.sleep(0)

        task.cancel()
        with self.assertRaises(asyncio.CancelledError):
            await task

        self.assertEqual(self.client.active_statuses, 0)
        self.assertLessEqual(self.client.max_active_statuses, 4)

    async def test_unexpected_status_failure_is_logged_with_device_context(self) -> None:
        self.client.errors[self.first.port] = RuntimeError("injected bug")
        with self.assertLogs("decky_power.controller", level="ERROR") as logs:
            result = await self.controller.statuses()

        self.assertEqual(result[self.first.id]["state"], DeviceState.UNKNOWN.value)
        self.assertTrue(any(self.first.id in message and "injected bug" in message for message in logs.output))


if __name__ == "__main__": unittest.main()
