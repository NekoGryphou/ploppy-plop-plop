from __future__ import annotations

from dataclasses import asdict, dataclass, field
from enum import StrEnum
from typing import Any


DEFAULT_PORT = 47_991
SCHEMA_VERSION = 2


class DeviceState(StrEnum):
    OFFLINE = "offline"
    STARTING = "starting"
    ONLINE = "online"
    STOPPING = "stopping"
    UNKNOWN = "unknown"
    AUTHENTICATION_FAILED = "authentication_failed"
    HOST_UNAVAILABLE = "host_unavailable"
    UPDATE_REQUIRED = "update_required"


@dataclass(slots=True)
class Device:
    id: str
    name: str
    address: str
    mac: str
    mac_overridden: bool = False
    port: int = DEFAULT_PORT
    broadcast_address: str | None = None
    host_id: str | None = None
    host_version: str | None = None
    protocol_version: int | None = None
    paired: bool = False

    def public_dict(self) -> dict[str, Any]:
        return asdict(self)


@dataclass(slots=True)
class Settings:
    schema_version: int = SCHEMA_VERSION
    devices: list[Device] = field(default_factory=list)

    def public_dict(self) -> dict[str, Any]:
        return {"schemaVersion": self.schema_version, "devices": [device.public_dict() for device in self.devices]}
