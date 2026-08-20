from __future__ import annotations

import json
import os
from pathlib import Path
from typing import Any
from uuid import uuid4

from .models import Device, SCHEMA_VERSION, Settings
from .validation import normalize_mac, validate_address, validate_broadcast, validate_port


class StoreError(ValueError):
    pass


class Store:
    def __init__(self, directory: str | Path):
        self.directory = Path(directory)
        self.settings_path = self.directory / "settings.json"
        self.secrets_path = self.directory / "credentials.json"
        self.directory.mkdir(parents=True, exist_ok=True)

    def load(self) -> Settings:
        if not self.settings_path.exists(): return Settings()
        try: raw = json.loads(self.settings_path.read_text("utf-8"))
        except (OSError, json.JSONDecodeError) as error: raise StoreError("Stored device configuration is corrupted.") from error
        version = raw.get("schemaVersion", 0)
        if version != SCHEMA_VERSION: raw = self._migrate(raw, version)
        try: return Settings(schema_version=SCHEMA_VERSION, devices=[Device(**self._snake(device)) for device in raw.get("devices", [])])
        except (TypeError, ValueError) as error: raise StoreError("Stored device configuration is invalid.") from error

    def save(self, settings: Settings) -> None:
        self._atomic_write(self.settings_path, settings.public_dict(), 0o600)

    def credentials(self) -> dict[str, bytes]:
        if not self.secrets_path.exists(): return {}
        try: return {key: bytes.fromhex(value) for key, value in json.loads(self.secrets_path.read_text("utf-8")).items()}
        except (OSError, ValueError, json.JSONDecodeError) as error: raise StoreError("Stored pairing credentials are corrupted.") from error

    def save_credentials(self, values: dict[str, bytes]) -> None:
        self._atomic_write(self.secrets_path, {key: value.hex() for key, value in values.items()}, 0o600)

    def upsert(self, values: dict[str, Any], device_id: str | None = None) -> Device:
        settings = self.load()
        device = Device(
            id=device_id or str(uuid4()), name=str(values.get("name", "")).strip(),
            address=validate_address(values.get("address")), mac=normalize_mac(str(values.get("mac", ""))),
            mac_overridden=bool(values.get("macOverridden", False)),
            port=validate_port(values.get("port"), default=True), broadcast_address=validate_broadcast(values.get("broadcastAddress")),
        )
        if not device.name: raise StoreError("Name is required.")
        existing = next((item for item in settings.devices if item.id == device.id), None)
        if existing:
            device.host_id, device.host_version, device.protocol_version, device.paired = existing.host_id, existing.host_version, existing.protocol_version, existing.paired
            settings.devices[settings.devices.index(existing)] = device
        else: settings.devices.append(device)
        self.save(settings); return device

    def delete(self, device_id: str) -> None:
        settings = self.load(); settings.devices = [device for device in settings.devices if device.id != device_id]; self.save(settings)
        credentials = self.credentials(); credentials.pop(device_id, None); self.save_credentials(credentials)

    def find(self, device_id: str) -> Device:
        device = next((item for item in self.load().devices if item.id == device_id), None)
        if device is None: raise StoreError("PC was not found.")
        return device

    def mark_paired(self, device_id: str, credential: bytes, host_id: str, hostname: str, host_version: str, protocol_version: int) -> Device:
        settings = self.load(); device = next(item for item in settings.devices if item.id == device_id)
        device.host_id, device.host_version, device.protocol_version, device.paired = host_id, host_version, protocol_version, True
        credentials = self.credentials(); credentials[device_id] = credential
        self.save_credentials(credentials); self.save(settings); return device

    @staticmethod
    def _snake(device: dict[str, Any]) -> dict[str, Any]:
        result = dict(device)
        for camel, snake in (("macOverridden", "mac_overridden"), ("broadcastAddress", "broadcast_address"), ("hostId", "host_id"), ("hostVersion", "host_version"), ("protocolVersion", "protocol_version")):
            if camel in result: result[snake] = result.pop(camel)
        return result

    @staticmethod
    def _migrate(raw: dict[str, Any], version: int) -> dict[str, Any]:
        if version == 0:
            raw = {"schemaVersion": 1, "devices": raw.get("devices", raw.get("rigs", []))}
            version = 1
        if version == 1:
            devices = raw.get("devices", [])
            for device in devices: device.setdefault("mac_overridden", True)
            return {"schemaVersion": 2, "devices": devices}
        raise StoreError(f"Settings schema version {version} is newer than this plugin supports.")

    @staticmethod
    def _atomic_write(path: Path, value: object, mode: int) -> None:
        temporary = path.with_suffix(path.suffix + ".tmp")
        temporary.write_text(json.dumps(value, indent=2) + "\n", "utf-8"); os.chmod(temporary, mode); os.replace(temporary, path)
