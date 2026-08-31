from __future__ import annotations

import asyncio
import socket
import tempfile
import unittest
from pathlib import Path

from decky_my_rig.client import HostClient
from decky_my_rig.controller import Controller
from decky_my_rig.models import DeviceState, PairingState
from decky_my_rig.store import Store


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
        host_writer: asyncio.StreamWriter | None = None
        try:
            host_reader, host_writer = await asyncio.open_connection("127.0.0.1", self.upstream_port)
            request = await client_reader.read(64 * 1024)
            host_writer.write(request)
            await host_writer.drain()
            headers = await host_reader.readuntil(b"\r\n\r\n")
            content_length = 0
            for header in headers.decode("ascii").split("\r\n"):
                if header.lower().startswith("content-length:"):
                    content_length = int(header.split(":", 1)[1].strip())
                    break
            response = headers + await host_reader.readexactly(content_length)
            await asyncio.sleep(self.response_delay)
            client_writer.write(response)
            await client_writer.drain()
        finally:
            if host_writer is not None:
                host_writer.close()
            client_writer.close()


class MultiPcE2ETests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.directory = Path(self.temporary.name)
        self.processes: list[asyncio.subprocess.Process] = []
        self.proxies: list[DelayedTcpProxy] = []

    async def asyncTearDown(self) -> None:
        for proxy in self.proxies: await proxy.close()
        for process in self.processes:
            if process.returncode is not None:
                continue
            process.terminate()
            try: await asyncio.wait_for(process.wait(), timeout=2)
            except TimeoutError:
                process.kill()
                await process.wait()
        self.temporary.cleanup()

    async def start_host(self, name: str, code: str) -> int:
        # Each process represents a different PC. Keep its configuration and
        # identity state in a separate directory, just as they would be on
        # separate machines; otherwise every process resolves the same
        # DeckyMyRigHost.dev-state.json sibling and races while starting.
        host_directory = self.directory / name
        host_directory.mkdir()
        config = host_directory / "DeckyMyRigHost.toml"
        config.write_text("port = 47991\n", "utf-8")
        executable = Path(__file__).parents[3] / "host" / "target" / "debug" / "decky-my-rig-host"
        process = await asyncio.create_subprocess_exec(
            str(executable), "--dev", "--mock-shutdown", "--ephemeral-port",
            "--config", str(config), "--pairing-code-value", code,
            stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.STDOUT,
        )
        self.processes.append(process)
        assert process.stdout is not None
        output: list[str] = []
        for _ in range(50):
            line = (await asyncio.wait_for(process.stdout.readline(), timeout=2)).decode("utf-8")
            output.append(line.rstrip())
            if line.startswith("DECKY_MY_RIG_LISTEN_PORT="): return int(line.split("=", 1)[1])
            if not line or process.returncode is not None:
                await process.wait()
                self.fail(f"{name} exited early ({process.returncode}): {' | '.join(output)}")
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
