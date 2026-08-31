import socket

from .validation import normalize_mac


def magic_packet(mac: str) -> bytes:
    raw = bytes.fromhex(normalize_mac(mac).replace(":", ""))
    return b"\xff" * 6 + raw * 16


def send_magic_packet(mac: str, broadcast_address: str | None = None, ports: tuple[int, ...] = (9, 7)) -> None:
    packet = magic_packet(mac); target = broadcast_address or "255.255.255.255"
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as connection:
        connection.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)
        for port in ports: connection.sendto(packet, (target, port))
