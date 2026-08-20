import tempfile
import unittest
from pathlib import Path

from decky_power.discovery import _find_proc_arp, find_mac


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


if __name__ == "__main__": unittest.main()
