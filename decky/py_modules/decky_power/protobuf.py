"""Small generated-equivalent codec for the v1 messages used by the backend.

The canonical schema is proto/decky_power.proto. Keeping this tiny codec avoids
shipping the full Python protobuf runtime in a Decky plugin. Unknown fields are
skipped per the Protobuf wire format.
"""

from dataclasses import dataclass


class DecodeError(ValueError):
    pass


def _varint(value: int) -> bytes:
    output = bytearray()
    while value > 0x7f:
        output.append((value & 0x7f) | 0x80); value >>= 7
    output.append(value)
    return bytes(output)


def uint(field: int, value: int) -> bytes:
    return _varint(field << 3) + _varint(value)


def blob(field: int, value: bytes) -> bytes:
    return _varint((field << 3) | 2) + _varint(len(value)) + value


def text(field: int, value: str) -> bytes:
    return blob(field, value.encode("utf-8"))


def _read_varint(data: bytes, offset: int) -> tuple[int, int]:
    value = 0
    for shift in range(0, 70, 7):
        if offset >= len(data): raise DecodeError("truncated varint")
        byte = data[offset]; offset += 1; value |= (byte & 0x7f) << shift
        if byte < 0x80: return value, offset
    raise DecodeError("oversized varint")


def fields(data: bytes) -> dict[int, int | bytes]:
    result: dict[int, int | bytes] = {}; offset = 0
    while offset < len(data):
        tag, offset = _read_varint(data, offset); number, wire = tag >> 3, tag & 7
        if number == 0: raise DecodeError("invalid field zero")
        if wire == 0: value, offset = _read_varint(data, offset)
        elif wire == 2:
            length, offset = _read_varint(data, offset); end = offset + length
            if end > len(data): raise DecodeError("truncated field")
            value, offset = data[offset:end], end
        elif wire == 1: value, offset = data[offset:offset + 8], offset + 8
        elif wire == 5: value, offset = data[offset:offset + 4], offset + 4
        else: raise DecodeError("unsupported wire type")
        result[number] = value
    return result


def string(value: int | bytes | None) -> str:
    if not isinstance(value, bytes): raise DecodeError("expected string")
    try: return value.decode("utf-8")
    except UnicodeDecodeError as error: raise DecodeError("invalid UTF-8") from error


@dataclass(frozen=True)
class StatusResponse:
    hostname: str; host_version: str; protocol_version: int; paired: bool; host_id: str

    @classmethod
    def decode(cls, data: bytes) -> "StatusResponse":
        values = fields(data)
        return cls(string(values.get(1)), string(values.get(2)), int(values.get(3, 0)), bool(values.get(4, 0)), string(values.get(5)))


@dataclass(frozen=True)
class PairResponse:
    host_message: bytes; nonce: bytes; ciphertext: bytes; hostname: str; host_version: str; protocol_version: int; host_id: str; session_id: bytes

    @classmethod
    def decode(cls, data: bytes) -> "PairResponse":
        values = fields(data)
        byte_value = lambda field: values.get(field) if isinstance(values.get(field), bytes) else b""
        return cls(byte_value(1), byte_value(2), byte_value(3), string(values.get(4)), string(values.get(5)), int(values.get(6, 0)), string(values.get(7)), byte_value(8))


def status_request() -> bytes: return uint(1, 1)
def shutdown_request() -> bytes: return uint(1, 1)
def pair_request(client_message: bytes = b"", session_id: bytes = b"", confirmation: bytes = b"") -> bytes:
    return uint(1, 1) + (blob(2, client_message) if client_message else b"") + (blob(3, session_id) if session_id else b"") + (blob(4, confirmation) if confirmation else b"")
