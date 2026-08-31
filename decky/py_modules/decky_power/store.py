from __future__ import annotations

import json
import os
import tempfile
import threading
import weakref
from pathlib import Path
from typing import Any
from uuid import uuid4

from .models import Device, SCHEMA_VERSION, Settings
from .validation import normalize_mac, validate_address, validate_broadcast, validate_port


class StoreError(ValueError):
    pass


class Store:
    _locks_guard = threading.Lock()
    _locks: weakref.WeakValueDictionary[Path, threading.RLock] = (
        weakref.WeakValueDictionary()
    )

    def __init__(self, directory: str | Path):
        self.directory = Path(directory).resolve()
        self.settings_path = self.directory / "settings.json"
        self.secrets_path = self.directory / "credentials.json"
        self.transaction_path = self.directory / "transaction.json"
        with self._locks_guard:
            self._lock = self._locks.setdefault(self.directory, threading.RLock())
        self.directory.mkdir(parents=True, exist_ok=True)
        with self._lock:
            self._recover_transaction()

    def load(self) -> Settings:
        with self._lock:
            self._recover_transaction()
            if not self.settings_path.exists():
                return Settings()
            return self._load_settings(self.credentials())

    def snapshot(self) -> tuple[Settings, dict[str, bytes]]:
        with self._lock:
            self._recover_transaction()
            credentials = self.credentials()
            settings = self._load_settings(credentials) if self.settings_path.exists() else Settings()
            return settings, credentials

    def _load_settings(self, credentials: dict[str, bytes]) -> Settings:
        try:
            raw = json.loads(self.settings_path.read_text("utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            raise StoreError("Stored device configuration is corrupted.") from error
        return self._decode_settings(raw, credentials)

    def _decode_settings(
        self, raw: object, credentials: dict[str, bytes]
    ) -> Settings:
        if not isinstance(raw, dict):
            raise StoreError("Stored device configuration is invalid.")
        version = raw.get("schemaVersion")
        if version != SCHEMA_VERSION:
            raise StoreError(f"Unsupported settings schema version: {version!r}.")
        try:
            settings = Settings(
                schema_version=SCHEMA_VERSION,
                devices=[
                    self._validated_device(device) for device in raw.get("devices", [])
                ],
            )
            for device in settings.devices:
                device.paired = device.id in credentials
            return settings
        except (TypeError, ValueError) as error:
            raise StoreError("Stored device configuration is invalid.") from error

    def save(self, settings: Settings) -> None:
        with self._lock:
            self._atomic_write(self.settings_path, settings.public_dict(), 0o600)

    def credentials(self) -> dict[str, bytes]:
        with self._lock:
            self._recover_transaction()
            if not self.secrets_path.exists():
                return {}
            try:
                document = json.loads(self.secrets_path.read_text("utf-8"))
                return self._decode_credentials(document)
            except (
                OSError,
                AttributeError,
                TypeError,
                ValueError,
                json.JSONDecodeError,
            ) as error:
                raise StoreError("Stored pairing credentials are corrupted.") from error

    @staticmethod
    def _decode_credentials(document: object) -> dict[str, bytes]:
        if not isinstance(document, dict):
            raise ValueError("credential document is not an object")
        values = {key: bytes.fromhex(value) for key, value in document.items()}
        if any(
            not isinstance(key, str) or not key or len(credential) != 32
            for key, credential in values.items()
        ):
            raise ValueError("invalid credential entry")
        return values

    def _save_credentials(self, values: dict[str, bytes]) -> None:
        with self._lock:
            self._atomic_write(
                self.secrets_path,
                {key: value.hex() for key, value in values.items()},
                0o600,
            )

    def upsert(self, values: dict[str, Any], device_id: str | None = None) -> Device:
        with self._lock:
            settings = self.load()
            device = Device(
                id=device_id or str(uuid4()),
                name=str(values.get("name", "")).strip(),
                address=validate_address(values.get("address")),
                mac=normalize_mac(str(values.get("mac", ""))),
                mac_overridden=bool(values.get("macOverridden", False)),
                port=validate_port(values.get("port"), default=True),
                broadcast_address=validate_broadcast(values.get("broadcastAddress")),
            )
            if not device.name:
                raise StoreError("Name is required.")
            existing = next((item for item in settings.devices if item.id == device.id), None)
            if existing:
                device.host_id = existing.host_id
                device.host_version = existing.host_version
                device.protocol_version = existing.protocol_version
                device.paired = existing.paired
                settings.devices[settings.devices.index(existing)] = device
            else:
                settings.devices.append(device)
            self.save(settings)
            return device

    def delete(self, device_id: str) -> None:
        with self._lock:
            previous_settings, previous_credentials = self.snapshot()
            settings = Settings(
                schema_version=previous_settings.schema_version,
                devices=[
                    device
                    for device in previous_settings.devices
                    if device.id != device_id
                ],
            )
            credentials = dict(previous_credentials)
            credentials.pop(device_id, None)
            self._commit(settings, credentials, previous_settings, previous_credentials)

    def find(self, device_id: str) -> Device:
        with self._lock:
            device = next((item for item in self.load().devices if item.id == device_id), None)
            if device is None:
                raise StoreError("PC was not found.")
            return device

    def mark_paired(
        self,
        device_id: str,
        credential: bytes,
        host_id: str,
        hostname: str,
        host_version: str,
        protocol_version: int,
    ) -> Device:
        del hostname  # Hostname is authenticated metadata; the configured display name remains user-owned.
        with self._lock:
            settings, previous_credentials = self.snapshot()
            previous_settings = Settings(
                schema_version=settings.schema_version,
                devices=[Device(**device.public_dict()) for device in settings.devices],
            )
            device = next(item for item in settings.devices if item.id == device_id)
            device.host_id = host_id
            device.host_version = host_version
            device.protocol_version = protocol_version
            device.paired = True
            credentials = dict(previous_credentials)
            credentials[device_id] = credential
            self._commit(settings, credentials, previous_settings, previous_credentials)
            return device

    def _commit(
        self,
        settings: Settings,
        credentials: dict[str, bytes],
        previous_settings: Settings,
        previous_credentials: dict[str, bytes],
    ) -> None:
        transaction = {
            "settings": settings.public_dict(),
            "credentials": {key: value.hex() for key, value in credentials.items()},
        }
        self._atomic_write(self.transaction_path, transaction, 0o600)
        credentials_written = False
        settings_written = False
        try:
            self._atomic_write(self.secrets_path, transaction["credentials"], 0o600)
            credentials_written = True
            self._atomic_write(self.settings_path, transaction["settings"], 0o600)
            settings_written = True
        except Exception:
            rollback_succeeded = True
            try:
                if credentials_written:
                    self._atomic_write(
                        self.secrets_path,
                        {key: value.hex() for key, value in previous_credentials.items()},
                        0o600,
                    )
                if settings_written:
                    self._atomic_write(
                        self.settings_path, previous_settings.public_dict(), 0o600
                    )
            except Exception:
                rollback_succeeded = False
            if rollback_succeeded:
                self._remove_transaction()
            raise
        self._remove_transaction()

    def _recover_transaction(self) -> None:
        if not self.transaction_path.exists():
            return
        try:
            transaction = json.loads(self.transaction_path.read_text("utf-8"))
            settings = transaction["settings"]
            credentials = transaction["credentials"]
            if not isinstance(settings, dict) or not isinstance(credentials, dict):
                raise TypeError("invalid transaction document")
            decoded_credentials = self._decode_credentials(credentials)
            self._decode_settings(settings, decoded_credentials)
            self._atomic_write(self.secrets_path, credentials, 0o600)
            self._atomic_write(self.settings_path, settings, 0o600)
            self._remove_transaction()
        except (
            OSError,
            KeyError,
            TypeError,
            ValueError,
            StoreError,
            json.JSONDecodeError,
        ) as error:
            raise StoreError("An interrupted settings transaction could not be recovered.") from error

    def _remove_transaction(self) -> None:
        try:
            self.transaction_path.unlink()
        except FileNotFoundError:
            return
        directory = os.open(self.directory, os.O_RDONLY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)

    @staticmethod
    def _snake(device: dict[str, Any]) -> dict[str, Any]:
        result = dict(device)
        for camel, snake in (("macOverridden", "mac_overridden"), ("broadcastAddress", "broadcast_address"), ("hostId", "host_id"), ("hostVersion", "host_version"), ("protocolVersion", "protocol_version")):
            if camel in result:
                result[snake] = result.pop(camel)
        return result

    @classmethod
    def _validated_device(cls, raw: dict[str, Any]) -> Device:
        device = Device(**cls._snake(raw))
        if not isinstance(device.id, str) or not device.id.strip():
            raise StoreError("Stored PC identifier is invalid.")
        if not isinstance(device.name, str):
            raise StoreError("Stored PC name is invalid.")
        device.name = device.name.strip()
        if not device.name:
            raise StoreError("Stored PC name is empty.")
        device.address = validate_address(device.address)
        device.mac = normalize_mac(device.mac)
        device.port = validate_port(device.port)
        device.broadcast_address = validate_broadcast(device.broadcast_address)
        return device

    @staticmethod
    def _atomic_write(path: Path, value: object, mode: int) -> None:
        temporary: str | None = None
        try:
            with tempfile.NamedTemporaryFile(
                "w",
                encoding="utf-8",
                dir=path.parent,
                prefix=f".{path.name}.",
                suffix=".tmp",
                delete=False,
            ) as output:
                temporary = output.name
                json.dump(value, output, indent=2)
                output.write("\n")
                output.flush()
                os.fsync(output.fileno())
            os.chmod(temporary, mode)
            os.replace(temporary, path)
            directory = os.open(path.parent, os.O_RDONLY)
            try:
                os.fsync(directory)
            finally:
                os.close(directory)
        finally:
            if temporary is not None:
                try:
                    os.unlink(temporary)
                except FileNotFoundError:
                    pass
