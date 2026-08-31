#!/usr/bin/env python3
"""Verify duplicated build metadata against repository sources of truth."""

from __future__ import annotations

import json
import re
import tomllib
from pathlib import Path


PROJECT = Path(__file__).resolve().parents[1]


def require(pattern: str, path: Path, label: str) -> str:
    match = re.search(pattern, path.read_text("utf-8"), re.MULTILINE)
    if match is None:
        raise RuntimeError(f"Could not find {label} in {path.relative_to(PROJECT)}")
    return match.group(1)


def main() -> int:
    cargo = tomllib.loads((PROJECT / "host/Cargo.toml").read_text("utf-8"))
    host_version = cargo["package"]["version"]
    package_version = json.loads((PROJECT / "decky/package.json").read_text("utf-8"))["version"]
    installer_version = require(r'^#define AppVersion "([^"]+)"', PROJECT / "host/installer/DeckyPowerHost.iss", "installer version")
    if len({host_version, package_version, installer_version}) != 1:
        raise RuntimeError(
            f"Release versions differ: Cargo={host_version}, npm={package_version}, installer={installer_version}"
        )

    python_plugin_version = require(
        r'^PLUGIN_VERSION = "([^"]+)"$',
        PROJECT / "decky/py_modules/decky_power/protobuf.py",
        "Python plugin version",
    )
    if python_plugin_version != package_version:
        raise RuntimeError(
            f"Plugin versions differ: npm={package_version}, Python={python_plugin_version}"
        )

    rust_protocol = require(r"^pub const PROTOCOL_VERSION: u32 = (\d+);", PROJECT / "host/src/lib.rs", "Rust protocol version")
    python_protocol = require(r"^PROTOCOL_VERSION = (\d+)$", PROJECT / "decky/py_modules/decky_power/protobuf.py", "Python protocol version")
    if rust_protocol != python_protocol:
        raise RuntimeError(f"Protocol versions differ: Rust={rust_protocol}, Python={python_protocol}")

    toolchain = tomllib.loads((PROJECT / "rust-toolchain.toml").read_text("utf-8"))["toolchain"]["channel"]
    pinned_locations = {
        "CI": require(r"toolchain: ([0-9.]+)", PROJECT / ".github/workflows/ci.yml", "CI Rust version"),
        "dev container": require(r"^ARG RUST_VERSION=([0-9.]+)$", PROJECT / ".devcontainer/Dockerfile", "dev-container Rust version"),
        "portable image": require(r"^FROM rust:([0-9.]+)-", PROJECT / "host/Dockerfile.portable", "portable-image Rust version"),
    }
    mismatches = {name: value for name, value in pinned_locations.items() if value != toolchain}
    if mismatches:
        details = ", ".join(f"{name}={value}" for name, value in mismatches.items())
        raise RuntimeError(f"Rust versions differ from rust-toolchain.toml ({toolchain}): {details}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
