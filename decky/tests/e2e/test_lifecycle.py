from __future__ import annotations

import asyncio
import socket
import subprocess
import tempfile
import unittest
from pathlib import Path
from typing import TextIO

from decky_power.client import HostClient, HostError
from decky_power.controller import Controller
from decky_power.models import DeviceState
from decky_power.store import Store


class LifecycleE2ETests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.directory = Path(self.temporary.name)
        self.store = Store(self.directory / "decky")
        self.host_process: subprocess.Popen[str] | None = None
        self.host_log_file: TextIO | None = None
        self.host_log_path: Path | None = None
        self.host_start_count = 0
        self.host_output = ""

    async def asyncTearDown(self) -> None:
        await self.stop_host()
        self.temporary.cleanup()

    async def start_host(self, code: str | None = "483921") -> int:
        config = self.directory / "DeckyPowerHost.toml"
        config.write_text("port = 47991\n", "utf-8")
        executable = Path(__file__).parents[3] / "host" / "target" / "debug" / "decky-power-host"
        arguments = [str(executable), "--dev", "--mock-shutdown", "--ephemeral-port", "--config", str(config)]
        if code is not None:
            arguments.extend(["--pairing-code-value", code])
        self.host_start_count += 1
        self.host_log_path = self.directory / f"host-{self.host_start_count}.log"
        self.host_log_file = self.host_log_path.open("w", encoding="utf-8")
        self.host_process = subprocess.Popen(
            arguments,
            stdout=self.host_log_file, stderr=subprocess.STDOUT, text=True,
        )
        for _ in range(100):
            self.host_log_file.flush()
            output = self.host_log_path.read_text("utf-8")
            for line in output.splitlines():
                if line.startswith("DECKY_POWER_LISTEN_PORT="):
                    return int(line.split("=", 1)[1])
            if self.host_process.poll() is not None:
                self.fail(f"portable host exited early: {output}")
            await asyncio.sleep(0.02)
        self.fail(f"portable host did not report its ephemeral port: {self.host_log_path.read_text('utf-8')}")

    async def stop_host(self) -> None:
        if self.host_process is None: return
        process = self.host_process
        process.terminate()
        try:
            await asyncio.to_thread(process.wait, timeout=2)
        except subprocess.TimeoutExpired:
            process.kill()
            await asyncio.to_thread(process.wait, timeout=2)
        if self.host_log_file is not None:
            self.host_log_file.close()
        if self.host_log_path is not None:
            self.host_output += self.host_log_path.read_text("utf-8")
        self.host_process = None
        self.host_log_file = None
        self.host_log_path = None

    async def test_create_later_pair_stop_offline_and_real_wol_datagram(self) -> None:
        device = self.store.upsert({
            "name": "Gaming PC", "address": "127.0.0.1", "port": 47991,
            "mac": "AA:BB:CC:DD:EE:FF", "macOverridden": True, "broadcastAddress": "127.0.0.1",
        })
        reloaded = Store(self.directory / "decky")
        self.assertEqual(len(reloaded.load().devices), 1)
        self.assertFalse(reloaded.find(device.id).paired)
        self.assertNotIn(device.id, reloaded.credentials())

        port = await self.start_host()
        device = reloaded.upsert({
            "name": device.name, "address": device.address, "port": port, "mac": device.mac,
            "macOverridden": True, "broadcastAddress": device.broadcast_address,
        }, device.id)
        client = HostClient(timeout=1)
        with self.assertRaises(HostError): await client.pair(device, "000000")
        self.assertEqual(len(reloaded.load().devices), 1)
        credential, response = await client.pair(device, "483921")
        paired = reloaded.mark_paired(device.id, credential, response.host_id, response.hostname, response.host_version, response.protocol_version)
        self.assertEqual(paired.id, device.id)
        self.assertEqual(len(Store(self.directory / "decky").load().devices), 1)

        # Restart both real process boundaries. The host must reload its identity
        # and credential, while the Deck backend must reload the same existing
        # device and optional credential rather than pairing or creating again.
        await self.stop_host()
        restarted_port = await self.start_host(code=None)
        restarted_store = Store(self.directory / "decky")
        restarted_device = restarted_store.upsert({
            "name": device.name, "address": device.address, "port": restarted_port, "mac": device.mac,
            "macOverridden": True, "broadcastAddress": device.broadcast_address,
        }, device.id)
        self.assertEqual(restarted_device.id, device.id)
        self.assertTrue(restarted_device.paired)
        self.assertEqual(restarted_store.credentials()[device.id], credential)

        # Re-pair the same persisted device through a newly generated
        # service-owned code. Its configuration and stable Deck-side ID remain,
        # while the previous long-term credential is invalidated.
        await self.stop_host()
        repaired_port = await self.start_host(code="654321")
        repaired_device = restarted_store.upsert({
            "name": device.name, "address": device.address, "port": repaired_port, "mac": device.mac,
            "macOverridden": True, "broadcastAddress": device.broadcast_address,
        }, device.id)
        replacement, repaired_response = await client.pair(repaired_device, "654321")
        restarted_store.mark_paired(device.id, replacement, repaired_response.host_id, repaired_response.hostname, repaired_response.host_version, repaired_response.protocol_version)
        self.assertNotEqual(replacement, credential)
        self.assertEqual(restarted_store.find(device.id).name, "Gaming PC")
        with self.assertRaises(HostError) as rejected_old:
            await client.status(repaired_device, credential)
        # A client holding the replaced key cannot cryptographically distinguish
        # the real host's unsigned authentication rejection from a forged LAN
        # error, so it must preserve pairing state and report integrity failure.
        self.assertEqual(rejected_old.exception.kind, "integrity")

        controller = Controller(restarted_store, HostClient(timeout=0.2), transition_timeout=0)
        self.assertEqual((await controller.status(device.id))["state"], DeviceState.ONLINE.value)
        self.assertEqual((await controller.stop(device.id))["state"], DeviceState.STOPPING.value)
        await self.stop_host()
        self.assertIn("Mock mode enabled: no system shutdown performed", self.host_output)
        self.assertEqual((await controller.status(device.id))["state"], DeviceState.OFFLINE.value)
        self.assertTrue(Store(self.directory / "decky").find(device.id).paired)

        receiver = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        receiver.bind(("127.0.0.1", 0))
        receiver.settimeout(1)
        wol_port = receiver.getsockname()[1]
        wol_controller = Controller(Store(self.directory / "decky"), wol_ports=(wol_port,))
        self.assertEqual((await wol_controller.start(device.id))["state"], DeviceState.STARTING.value)
        packet, _ = await asyncio.to_thread(receiver.recvfrom, 2048)
        receiver.close()
        self.assertEqual(packet, b"\xff" * 6 + bytes.fromhex("AABBCCDDEEFF") * 16)


if __name__ == "__main__": unittest.main()
