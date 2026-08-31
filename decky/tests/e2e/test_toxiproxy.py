from __future__ import annotations

import asyncio
import json
import os
import time
import unittest
from urllib.error import HTTPError
from urllib.request import Request, urlopen

from decky_power.client import HostClient, HostError
from decky_power.models import Device


@unittest.skipUnless(os.environ.get("DECKY_POWER_TOXIPROXY") == "1", "requires Docker Compose Toxiproxy topology")
class ToxiproxyE2ETests(unittest.IsolatedAsyncioTestCase):
    credential: bytes | None = None
    host_id: str | None = None

    def device(self, port: int) -> Device:
        return Device(id=str(port), name="Toxiproxy PC", address="127.0.0.1", mac="AA:BB:CC:DD:EE:FF", port=port)

    async def api(self, method: str, path: str, payload: dict | None = None) -> None:
        body = None if payload is None else json.dumps(payload).encode()
        request = Request(f"http://127.0.0.1:58474{path}", data=body, method=method, headers={"Content-Type": "application/json"})
        def send() -> None:
            try: urlopen(request, timeout=2).close()
            except HTTPError as error:
                error.close()
                raise
        await asyncio.to_thread(send)

    async def asyncTearDown(self) -> None:
        for name in ("latency", "large-latency", "interrupt-request", "interrupt-response"):
            try: await self.api("DELETE", f"/proxies/host-slow/toxics/{name}")
            except Exception: pass
        try: await self.api("PATCH", "/proxies/host-slow", {"enabled": True})
        except Exception: pass

    async def paired_through_proxy(self) -> tuple[Device, bytes]:
        if type(self).credential is None:
            direct = self.device(58201)
            type(self).credential, response = await HostClient(timeout=2).pair(direct, "333333")
            type(self).host_id = response.host_id
        proxied = self.device(58200); proxied.host_id = type(self).host_id
        assert type(self).credential is not None
        return proxied, type(self).credential

    async def test_normal_and_real_latency(self) -> None:
        device, credential = await self.paired_through_proxy()
        self.assertTrue((await HostClient(timeout=1).status(device, credential)).hostname)
        await self.api("POST", "/proxies/host-slow/toxics", {"name": "latency", "type": "latency", "stream": "downstream", "attributes": {"latency": 75, "jitter": 0}})
        started = time.monotonic()
        self.assertTrue((await HostClient(timeout=1).status(device, credential)).hostname)
        self.assertGreaterEqual(time.monotonic() - started, 0.06)

    async def test_large_latency_causes_real_network_timeout(self) -> None:
        device, credential = await self.paired_through_proxy()
        await self.api("POST", "/proxies/host-slow/toxics", {"name": "large-latency", "type": "latency", "stream": "downstream", "attributes": {"latency": 500, "jitter": 0}})
        started = time.monotonic()
        with self.assertRaises(HostError): await HostClient(timeout=0.08).status(device, credential)
        self.assertLess(time.monotonic() - started, 0.4)

    async def test_disabled_proxy_refuses_or_resets_connection(self) -> None:
        device, credential = await self.paired_through_proxy()
        await self.api("PATCH", "/proxies/host-slow", {"enabled": False})
        with self.assertRaises(HostError): await HostClient(timeout=0.2).status(device, credential)

    async def test_response_interruption_is_reported(self) -> None:
        device, credential = await self.paired_through_proxy()
        await self.api("POST", "/proxies/host-slow/toxics", {"name": "interrupt-response", "type": "limit_data", "stream": "downstream", "attributes": {"bytes": 12}})
        with self.assertRaises((HostError, ValueError)): await HostClient(timeout=0.2).status(device, credential)

    async def test_request_interruption_is_reported(self) -> None:
        device, credential = await self.paired_through_proxy()
        await self.api("POST", "/proxies/host-slow/toxics", {"name": "interrupt-request", "type": "limit_data", "stream": "upstream", "attributes": {"bytes": 20}})
        with self.assertRaises(HostError): await HostClient(timeout=0.2).status(device, credential)


if __name__ == "__main__": unittest.main()
