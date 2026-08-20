import json
import tempfile
import unittest
from pathlib import Path

from decky_power.store import Store, StoreError


class StoreTests(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.store = Store(self.directory.name)

    def tearDown(self) -> None: self.directory.cleanup()

    def test_multiple_devices_keep_stable_ids_and_ports(self) -> None:
        first = self.store.upsert({"name": "Gaming", "address": "gaming.local", "mac": "AA-BB-CC-DD-EE-FF", "port": "47991", "broadcastAddress": ""})
        second = self.store.upsert({"name": "Bedroom", "address": "192.168.1.42", "mac": "11:22:33:44:55:66", "port": "48100", "broadcastAddress": "192.168.1.255"})
        edited = self.store.upsert({"name": "Gaming PC", "address": "gaming.local", "mac": first.mac, "port": "49000", "broadcastAddress": ""}, first.id)
        self.assertEqual(edited.id, first.id)
        self.assertEqual([device.port for device in self.store.load().devices], [49000, 48100])
        self.assertNotEqual(first.id, second.id)

    def test_credentials_are_separate_and_mode_0600(self) -> None:
        device = self.store.upsert({"name": "Gaming", "address": "gaming.local", "mac": "AABBCCDDEEFF", "port": 47991, "broadcastAddress": ""})
        self.store.mark_paired(device.id, b"x" * 32, "host", "gaming", "1.0.0", 1)
        self.assertNotIn("7878", self.store.settings_path.read_text("utf-8"))
        self.assertEqual(self.store.secrets_path.stat().st_mode & 0o777, 0o600)

    def test_corruption_and_future_schema_fail_clearly(self) -> None:
        self.store.settings_path.write_text("{", "utf-8")
        with self.assertRaises(StoreError): self.store.load()
        self.store.settings_path.write_text(json.dumps({"schemaVersion": 99, "devices": []}), "utf-8")
        with self.assertRaises(StoreError): self.store.load()

    def test_version_zero_migrates(self) -> None:
        Path(self.store.settings_path).write_text(json.dumps({"rigs": []}), "utf-8")
        self.assertEqual(self.store.load().schema_version, 2)

    def test_version_one_migrates_existing_mac_to_manual_override(self) -> None:
        device = {"id": "old", "name": "Old PC", "address": "old.local", "mac": "AA:BB:CC:DD:EE:FF", "port": 47991}
        self.store.settings_path.write_text(json.dumps({"schemaVersion": 1, "devices": [device]}), "utf-8")
        self.assertTrue(self.store.load().devices[0].mac_overridden)


if __name__ == "__main__": unittest.main()
