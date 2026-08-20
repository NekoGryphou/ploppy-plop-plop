from __future__ import annotations

import asyncio
import ipaddress
import re
import socket
from pathlib import Path

from .validation import normalize_mac, validate_address, validate_port


MAC_PATTERN = re.compile(r"(?i)\b([0-9a-f]{2}(?:[:-][0-9a-f]{2}){5})\b")


class DiscoveryError(ValueError):
    pass


async def discover_mac(address: object, port: object) -> str:
    """Resolve a host and retrieve its MAC from SteamOS' IPv4 neighbor table."""
    host = validate_address(address)
    host_port = validate_port(port, default=True)
    ip = await _resolve_ipv4(host)
    await _prime_neighbor(ip, host_port)

    proc_mac = _find_proc_arp(ip, Path("/proc/net/arp"))
    if proc_mac:
        return proc_mac

    try:
        process = await asyncio.create_subprocess_exec(
            "ip", "neigh", "show", ip,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.DEVNULL,
        )
        stdout, _ = await asyncio.wait_for(process.communicate(), timeout=3)
    except (FileNotFoundError, TimeoutError):
        stdout = b""
    mac = find_mac(ip, stdout.decode("utf-8", errors="replace"))
    if mac:
        return mac
    raise DiscoveryError("MAC address was not found. Make sure the PC is powered on and on the same LAN, or enter it manually.")


async def _resolve_ipv4(host: str) -> str:
    try:
        return str(ipaddress.IPv4Address(host))
    except ipaddress.AddressValueError:
        pass
    try:
        values = await asyncio.get_running_loop().getaddrinfo(host, None, family=socket.AF_INET, type=socket.SOCK_STREAM)
    except socket.gaierror as error:
        raise DiscoveryError("The PC address could not be resolved.") from error
    if not values:
        raise DiscoveryError("The PC address has no IPv4 address.")
    return str(values[0][4][0])


async def _prime_neighbor(ip: str, port: int) -> None:
    try:
        _reader, writer = await asyncio.wait_for(asyncio.open_connection(ip, port), timeout=1.5)
        writer.close()
        await writer.wait_closed()
    except (OSError, TimeoutError):
        # Even a refused or timed-out LAN connection can populate the ARP table.
        return


def _find_proc_arp(ip: str, path: Path) -> str:
    try:
        return find_mac(ip, path.read_text("utf-8"))
    except OSError:
        return ""


def find_mac(ip: str, output: str) -> str:
    """Extract and normalize a MAC only from a line belonging to the requested IP."""
    for line in output.splitlines():
        fields = line.split()
        if ip not in fields:
            continue
        match = MAC_PATTERN.search(line)
        if match:
            return normalize_mac(match.group(1))
    return ""
