from __future__ import annotations

import re
from enum import StrEnum


_VERSION = re.compile(r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$")


class VersionRelation(StrEnum):
    COMPATIBLE = "compatible"
    UPDATE_HOST = "update_host"
    UPDATE_PLUGIN = "update_plugin"
    INCOMPATIBLE = "incompatible"
    UNKNOWN = "unknown"


def compare_versions(plugin_version: str, host_version: str) -> VersionRelation:
    plugin = _VERSION.fullmatch(plugin_version)
    host = _VERSION.fullmatch(host_version)
    if plugin is None or host is None:
        return VersionRelation.UNKNOWN
    plugin_parts = tuple(map(int, plugin.groups()))
    host_parts = tuple(map(int, host.groups()))
    if plugin_parts[0] != host_parts[0]:
        return VersionRelation.INCOMPATIBLE
    if plugin_parts[1] > host_parts[1]:
        return VersionRelation.UPDATE_HOST
    if plugin_parts[1] < host_parts[1]:
        return VersionRelation.UPDATE_PLUGIN
    return VersionRelation.COMPATIBLE
