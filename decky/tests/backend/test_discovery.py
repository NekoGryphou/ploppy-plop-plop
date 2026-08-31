import asyncio
import tempfile
import unittest
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock, patch

from decky_my_rig.discovery import DiscoveryError, _find_proc_arp, discover_mac, find_mac


class DiscoveryTests(unittest.TestCase):
    def test_finds_linux_neighbor_mac_for_only_requested_ip(self) -> None:
        output = "192.168.1.20 dev wlan0 lladdr aa-bb-cc-dd-ee-ff REACHABLE\n192.168.1.21 dev wlan0 lladdr 11:22:33:44:55:66 STALE"
        self.assertEqual(find_mac("192.168.1.20", output), "AA:BB:CC:DD:EE:FF")
        self.assertEqual(find_mac("192.168.1.99", output), "")

    def test_reads_proc_arp_format(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "arp"
            path.write_text("IP address HW type Flags HW address Mask Device\n192.168.1.42 0x1 0x2 11:22:33:44:55:66 * wlan0\n", "utf-8")
            self.assertEqual(_find_proc_arp("192.168.1.42", path), "11:22:33:44:55:66")

    def test_rejects_invalid_neighbor_mac(self) -> None:
        self.assertEqual(find_mac("192.168.1.42", "192.168.1.42 dev wlan0 FAILED"), "")

    def test_timed_out_neighbor_process_is_killed_and_reaped(self) -> None:
        async def timeout(awaitable, *, timeout):
            del timeout
            awaitable.close()
            raise TimeoutError

        process = MagicMock()
        process.communicate = AsyncMock()
        process.wait = AsyncMock()
        with (
            patch("decky_my_rig.discovery._resolve_ipv4", new=AsyncMock(return_value="192.168.1.42")),
            patch("decky_my_rig.discovery._prime_neighbor", new=AsyncMock()),
            patch("decky_my_rig.discovery._find_proc_arp", return_value=""),
            patch("decky_my_rig.discovery.asyncio.create_subprocess_exec", new=AsyncMock(return_value=process)),
            patch("decky_my_rig.discovery.asyncio.wait_for", new=timeout),
        ):
            with self.assertRaises(DiscoveryError):
                asyncio.run(discover_mac("pc.local", 47991))

        process.kill.assert_called_once_with()
        process.wait.assert_awaited_once_with()


if __name__ == "__main__": unittest.main()
