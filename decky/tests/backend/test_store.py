import json
import tempfile
import concurrent.futures
import unittest
from pathlib import Path
from unittest.mock import patch

from decky_my_rig.store import Store, StoreError


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

    def test_plugin_upgrade_reopens_existing_pairing_without_rotation(self) -> None:
        device = self.store.upsert({"name": "Gaming", "address": "gaming.local", "mac": "AABBCCDDEEFF", "port": 47991, "broadcastAddress": ""})
        credential = bytes(range(32))
        self.store.mark_paired(device.id, credential, "stable-host-id", "gaming", "0.1.0", 1)

        upgraded_process_store = Store(self.directory.name)
        reloaded = upgraded_process_store.find(device.id)

        self.assertTrue(reloaded.paired)
        self.assertEqual(reloaded.host_id, "stable-host-id")
        self.assertEqual(upgraded_process_store.credentials()[device.id], credential)

    def test_unpaired_device_persists_without_credentials(self) -> None:
        created = self.store.upsert({"name": "Offline PC", "address": "offline.local", "mac": "AABBCCDDEEFF", "port": 47991, "broadcastAddress": ""})

        reloaded = Store(self.directory.name).find(created.id)

        self.assertFalse(reloaded.paired)
        self.assertIsNone(reloaded.host_id)
        self.assertNotIn(created.id, Store(self.directory.name).credentials())

    def test_unpaired_device_can_be_edited_and_deleted(self) -> None:
        created = self.store.upsert({"name": "Offline PC", "address": "offline.local", "mac": "AABBCCDDEEFF", "port": 47991, "broadcastAddress": ""})
        edited = self.store.upsert({"name": "Renamed PC", "address": "offline.local", "mac": created.mac, "port": 48100, "broadcastAddress": ""}, created.id)
        self.assertEqual(edited.id, created.id)
        self.assertFalse(edited.paired)
        self.assertEqual(edited.port, 48100)
        self.store.delete(created.id)
        with self.assertRaisesRegex(StoreError, "not found"):
            self.store.find(created.id)

    def test_failed_pairing_metadata_write_does_not_leave_a_credential(self) -> None:
        device = self.store.upsert({"name": "Offline PC", "address": "offline.local", "mac": "AABBCCDDEEFF", "port": 47991, "broadcastAddress": ""})
        original_write = self.store._atomic_write

        def fail_settings(path: Path, value: object, mode: int) -> None:
            if path == self.store.settings_path:
                raise OSError("simulated settings write failure")
            original_write(path, value, mode)

        with patch.object(self.store, "_atomic_write", side_effect=fail_settings):
            with self.assertRaises(OSError):
                self.store.mark_paired(device.id, b"x" * 32, "host", "gaming", "1.0.0", 1)

        self.assertNotIn(device.id, self.store.credentials())
        self.assertFalse(self.store.find(device.id).paired)

    def test_concurrent_device_updates_are_not_lost(self) -> None:
        def add(index: int) -> None:
            self.store.upsert({"name": f"PC {index}", "address": f"pc-{index}.local", "mac": f"AABBCCDD{index:04X}", "port": 47991, "broadcastAddress": ""})

        with concurrent.futures.ThreadPoolExecutor(max_workers=8) as workers:
            list(workers.map(add, range(20)))

        self.assertEqual(len(self.store.load().devices), 20)

    def test_failed_delete_restores_the_credential(self) -> None:
        device = self.store.upsert({"name": "Gaming", "address": "gaming.local", "mac": "AABBCCDDEEFF", "port": 47991, "broadcastAddress": ""})
        self.store.mark_paired(device.id, b"x" * 32, "host", "gaming", "1.0.0", 1)
        original_write = self.store._atomic_write

        def fail_settings(path: Path, value: object, mode: int) -> None:
            if path == self.store.settings_path:
                raise OSError("simulated settings write failure")
            original_write(path, value, mode)

        with patch.object(self.store, "_atomic_write", side_effect=fail_settings):
            with self.assertRaises(OSError):
                self.store.delete(device.id)

        self.assertEqual(self.store.credentials()[device.id], b"x" * 32)
        self.assertTrue(self.store.find(device.id).paired)

    def test_interrupted_transaction_rolls_forward_on_restart(self) -> None:
        device = self.store.upsert({"name": "Gaming", "address": "gaming.local", "mac": "AABBCCDDEEFF", "port": 47991, "broadcastAddress": ""})
        updated = self.store.load().public_dict()
        updated["devices"][0]["host_id"] = "replacement-host"
        transaction = {"settings": updated, "credentials": {device.id: (b"z" * 32).hex()}}
        self.store._atomic_write(self.store.transaction_path, transaction, 0o600)
        self.store._atomic_write(self.store.secrets_path, transaction["credentials"], 0o600)

        recovered = Store(self.directory.name)

        self.assertFalse(recovered.transaction_path.exists())
        self.assertEqual(recovered.credentials()[device.id], b"z" * 32)
        self.assertEqual(recovered.find(device.id).host_id, "replacement-host")

    def test_invalid_transaction_never_overwrites_last_good_state(self) -> None:
        device = self.store.upsert({"name": "Gaming", "address": "gaming.local", "mac": "AABBCCDDEEFF", "port": 47991, "broadcastAddress": ""})
        self.store._atomic_write(
            self.store.transaction_path,
            {"settings": {"schemaVersion": 2, "devices": "invalid"}, "credentials": {device.id: "00"}},
            0o600,
        )

        with self.assertRaisesRegex(StoreError, "transaction could not be recovered"):
            Store(self.directory.name)

        self.store.transaction_path.unlink()
        self.assertEqual(Store(self.directory.name).find(device.id).name, "Gaming")

    def test_corruption_and_future_schema_fail_clearly(self) -> None:
        self.store.settings_path.write_text("{", "utf-8")
        with self.assertRaises(StoreError): self.store.load()

    def test_invalid_persisted_network_configuration_is_rejected(self) -> None:
        base = {"schemaVersion": 2, "devices": [{"id": "bad", "name": "Bad", "address": "bad.local", "mac": "AA:BB:CC:DD:EE:FF", "port": 0}]}
        for field, value in (
            ("id", ""),
            ("name", 42),
            ("port", 0),
            ("port", 65536),
            ("mac", "not-a-mac"),
            ("address", "http://bad.local"),
        ):
            with self.subTest(field=field, value=value):
                document = json.loads(json.dumps(base))
                document["devices"][0][field] = value
                self.store.settings_path.write_text(json.dumps(document), "utf-8")
                with self.assertRaises(StoreError):
                    self.store.load()

    def test_malformed_credential_length_is_rejected(self) -> None:
        self.store.secrets_path.write_text(json.dumps({"device": "00"}), "utf-8")
        with self.assertRaisesRegex(StoreError, "credentials are corrupted"):
            self.store.credentials()
        self.store.settings_path.write_text(json.dumps({"schemaVersion": 99, "devices": []}), "utf-8")
        with self.assertRaises(StoreError): self.store.load()

    def test_pre_release_schema_versions_are_rejected_without_compatibility(self) -> None:
        for document in ({"rigs": []}, {"schemaVersion": 1, "devices": []}):
            with self.subTest(document=document):
                Path(self.store.settings_path).write_text(json.dumps(document), "utf-8")
                with self.assertRaisesRegex(StoreError, "Unsupported settings schema"):
                    self.store.load()


if __name__ == "__main__": unittest.main()
