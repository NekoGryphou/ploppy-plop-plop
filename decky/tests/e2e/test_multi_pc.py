from __future__ import annotations

import asyncio
import socket
import subprocess
import tempfile
import unittest
from pathlib import Path

from decky_power.client import HostClient
from decky_power.controller import Controller
from decky_power.models import DeviceState, PairingState
from decky_power.store import Store


class DelayedTcpProxy:
    def __init__(self, upstream_port: int, response_delay: float):
        self.upstream_port = upstream_port
        self.response_delay = response_delay
        self.server: asyncio.Server | None = None

    async def start(self) -> int:
        self.server = await asyncio.start_server(self._handle, "127.0.0.1", 0)
        return self.server.sockets[0].getsockname()[1]

    async def close(self) -> None:
        if self.server is not None:
            self.server.close()
            await self.server.wait_closed()

    async def _handle(self, client_reader: asyncio.StreamReader, client_writer: asyncio.StreamWriter) -> None:
        try:
            host_reader, host_writer = await asyncio.open_connection("127.0.0.1", self.upstream_port)
            request = await client_reader.read(64 * 1024)
            host_writer.write(request)
            await host_writer.drain()
            response = await host_reader.read(64 * 1024)
            await asyncio.sleep(self.response_delay)
            client_writer.write(response)
            await client_writer.drain()
            host_writer.close()
            await host_writer.wait_closed()
        finally:
            client_writer.close()
            await client_writer.wait_closed()


class MultiPcE2ETests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.directory = Path(self.temporary.name)
        self.processes: list[subprocess.Popen[str]] = []
        self.proxies: list[DelayedTcpProxy] = []

    async def asyncTearDown(self) -> None:
        for proxy in self.proxies: await proxy.close()
        for process in self.processes:
            process.terminate()
            try: await asyncio.wait_for(asyncio.to_thread(process.wait), timeout=2)
            except TimeoutError:
                process.kill()
                await asyncio.to_thread(process.wait)
            if process.stdout is not None: process.stdout.close()
        self.temporary.cleanup()

    async def start_host(self, name: str, code: str) -> int:
        config = self.directory / f"{name}.toml"
        config.write_text("port = 47991\n", "utf-8")
        executable = Path(__file__).parents[3] / "host" / "target" / "debug" / "decky-power-host"
        process = subprocess.Popen(
            [str(executable), "--dev", "--mock-shutdown", "--ephemeral-port", "--config", str(config), "--pairing-code-value", code],
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
        )
        self.processes.append(process)
        assert process.stdout is not None
        for _ in range(50):
            line = await asyncio.wait_for(asyncio.to_thread(process.stdout.readline), timeout=2)
            if line.startswith("DECKY_POWER_LISTEN_PORT="): return int(line.split("=", 1)[1])
            if process.poll() is not None: self.fail(f"{name} exited early: {line}")
        self.fail(f"{name} did not report a port")

    @staticmethod
    def unavailable_port() -> int:
        probe = socket.socket()
        probe.bind(("127.0.0.1", 0))
        port = probe.getsockname()[1]
        probe.close()
        return port

    async def test_paired_unpaired_slow_and_unreachable_hosts_are_isolated(self) -> None:
        port_a, port_b, port_c = await asyncio.gather(
            self.start_host("host-a", "111111"),
            self.start_host("host-b", "222222"),
            self.start_host("host-c", "333333"),
        )
        proxy = DelayedTcpProxy(port_c, response_delay=0.25)
        self.proxies.append(proxy)
        slow_port = await proxy.start()
        store = Store(self.directory / "decky")
        devices = [
            store.upsert({"name": "PC A", "address": "127.0.0.1", "port": port_a, "mac": "001122334455", "macOverridden": True, "broadcastAddress": ""}),
            store.upsert({"name": "PC B", "address": "127.0.0.1", "port": port_b, "mac": "112233445566", "macOverridden": True, "broadcastAddress": ""}),
            store.upsert({"name": "PC C", "address": "127.0.0.1", "port": slow_port, "mac": "223344556677", "macOverridden": True, "broadcastAddress": ""}),
            store.upsert({"name": "PC D", "address": "127.0.0.1", "port": self.unavailable_port(), "mac": "334455667788", "macOverridden": True, "broadcastAddress": ""}),
        ]
        setup_client = HostClient(timeout=1)
        credential_a, response_a = await setup_client.pair(devices[0], "111111")
        store.mark_paired(devices[0].id, credential_a, response_a.host_id, response_a.hostname, response_a.host_version, response_a.protocol_version)
        direct_c = store.upsert({"name": "PC C", "address": "127.0.0.1", "port": port_c, "mac": devices[2].mac, "macOverridden": True, "broadcastAddress": ""}, devices[2].id)
        credential_c, response_c = await setup_client.pair(direct_c, "333333")
        store.mark_paired(direct_c.id, credential_c, response_c.host_id, response_c.hostname, response_c.host_version, response_c.protocol_version)
        store.upsert({"name": "PC C", "address": "127.0.0.1", "port": slow_port, "mac": devices[2].mac, "macOverridden": True, "broadcastAddress": ""}, devices[2].id)

        controller = Controller(Store(self.directory / "decky"), HostClient(timeout=0.08))
        results = await asyncio.wait_for(controller.statuses(), timeout=0.5)

        self.assertEqual(results[devices[0].id]["state"], DeviceState.ONLINE.value)
        self.assertEqual(results[devices[1].id]["state"], DeviceState.ONLINE.value)
        self.assertEqual(results[devices[1].id]["pairing"], PairingState.UNPAIRED.value)
        self.assertEqual(results[devices[2].id]["state"], DeviceState.OFFLINE.value)
        self.assertEqual(results[devices[3].id]["state"], DeviceState.OFFLINE.value)
        self.assertEqual([device.port for device in store.load().devices], [port_a, port_b, slow_port, devices[3].port])


if __name__ == "__main__": unittest.main()
