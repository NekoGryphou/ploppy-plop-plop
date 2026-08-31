"""Small schema-driven codec for the v1 messages used by the backend.

Field names, numbers, and types are generated from proto/decky_my_rig.proto.
Keeping the wire runtime tiny avoids shipping the full Python protobuf runtime
in a Decky plugin. Unknown fields are skipped per the Protobuf wire format.
"""

from dataclasses import dataclass

from .generated_schema import MESSAGES

PROTOCOL_VERSION = 1
PLUGIN_VERSION = "0.1.0"


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


def fields(data: bytes) -> dict[int, tuple[int, int | bytes]]:
    result: dict[int, tuple[int, int | bytes]] = {}; offset = 0
    while offset < len(data):
        tag, offset = _read_varint(data, offset); number, wire = tag >> 3, tag & 7
        if number == 0: raise DecodeError("invalid field zero")
        if wire == 0: value, offset = _read_varint(data, offset)
        elif wire == 2:
            length, offset = _read_varint(data, offset); end = offset + length
            if end > len(data): raise DecodeError("truncated field")
            value, offset = data[offset:end], end
        elif wire in (1, 5):
            width = 8 if wire == 1 else 4
            end = offset + width
            if end > len(data): raise DecodeError("truncated fixed-width field")
            value, offset = data[offset:end], end
        else: raise DecodeError("unsupported wire type")
        result[number] = (wire, value)
    return result


def string(value: int | bytes | None) -> str:
    if not isinstance(value, bytes): raise DecodeError("expected string")
    try: return value.decode("utf-8")
    except UnicodeDecodeError as error: raise DecodeError("invalid UTF-8") from error


def decode_message(message: str, data: bytes) -> dict[str, int | bytes | str | bool]:
    encoded = fields(data)
    decoded: dict[str, int | bytes | str | bool] = {}
    for name, (number, field_type) in MESSAGES[message].items():
        if number not in encoded:
            continue
        wire, value = encoded[number]
        expected_wire = 2 if field_type in ("string", "bytes") else 0
        if wire != expected_wire:
            raise DecodeError(f"invalid wire type for {message}.{name}")
        if field_type == "string":
            decoded[name] = string(value)
        elif field_type == "bytes":
            if not isinstance(value, bytes): raise DecodeError("expected bytes")
            decoded[name] = value
        elif field_type == "bool":
            if not isinstance(value, int): raise DecodeError("expected boolean")
            decoded[name] = bool(value)
        else:
            if not isinstance(value, int): raise DecodeError("expected integer")
            decoded[name] = value
    return decoded


def encode_message(message: str, values: dict[str, int | bytes | str | bool]) -> bytes:
    output = bytearray()
    for name, value in values.items():
        number, field_type = MESSAGES[message][name]
        if field_type == "string":
            output.extend(text(number, str(value)))
        elif field_type == "bytes":
            output.extend(blob(number, bytes(value)))
        else:
            output.extend(uint(number, int(value)))
    return bytes(output)


def _required(values: dict[str, int | bytes | str | bool], name: str, expected: type) -> int | bytes | str | bool:
    value = values.get(name)
    if not isinstance(value, expected) or value in ("", b""):
        raise DecodeError(f"missing or invalid field: {name}")
    return value


@dataclass(frozen=True)
class StatusResponse:
    hostname: str; host_version: str; protocol_version: int; paired: bool; host_id: str

    @classmethod
    def decode(cls, data: bytes) -> "StatusResponse":
        values = decode_message("StatusResponse", data)
        return cls(str(_required(values, "hostname", str)), str(_required(values, "host_version", str)), int(values.get("protocol_version", 0)), bool(values.get("paired", False)), str(_required(values, "host_id", str)))


@dataclass(frozen=True)
class PairResponse:
    host_message: bytes; nonce: bytes; ciphertext: bytes; hostname: str; host_version: str; protocol_version: int; host_id: str; session_id: bytes

    @classmethod
    def decode(cls, data: bytes) -> "PairResponse":
        values = decode_message("PairResponse", data)
        byte_value = lambda name: value if isinstance((value := values.get(name)), bytes) else b""
        return cls(bytes(_required(values, "host_spake2_message", bytes)), byte_value("encryption_nonce"), byte_value("encrypted_credential"), str(_required(values, "hostname", str)), str(_required(values, "host_version", str)), int(values.get("protocol_version", 0)), str(_required(values, "host_id", str)), bytes(_required(values, "session_id", bytes)))


def pairing_credential_aad(response: PairResponse) -> bytes:
    output = bytearray(b"deckymyrig-pairing-credential-aad-v1\0")
    for field in (
        response.host_message,
        response.session_id,
        response.hostname.encode("utf-8"),
        response.host_version.encode("utf-8"),
        response.host_id.encode("utf-8"),
    ):
        if len(field) > 65_535: raise DecodeError("pairing metadata is too large")
        output.extend(len(field).to_bytes(2, "big"))
        output.extend(field)
    output.extend(response.protocol_version.to_bytes(4, "big"))
    return bytes(output)


@dataclass(frozen=True)
class ErrorResponse:
    code: int; message: str

    @classmethod
    def decode(cls, data: bytes) -> "ErrorResponse":
        values = decode_message("ErrorResponse", data)
        return cls(int(values.get("code", 0)), str(values.get("message", "")))


def status_request() -> bytes: return encode_message("StatusRequest", {"protocol_version": PROTOCOL_VERSION, "client_version": PLUGIN_VERSION})
def shutdown_request() -> bytes: return encode_message("ShutdownRequest", {"protocol_version": PROTOCOL_VERSION, "client_version": PLUGIN_VERSION})
def pair_request(client_message: bytes = b"", session_id: bytes = b"", confirmation: bytes = b"") -> bytes:
    values: dict[str, int | bytes | str | bool] = {"protocol_version": PROTOCOL_VERSION, "client_version": PLUGIN_VERSION}
    if client_message: values["client_spake2_message"] = client_message
    if session_id: values["session_id"] = session_id
    if confirmation: values["client_confirmation"] = confirmation
    return encode_message("PairRequest", values)
