import ipaddress
import re

from .models import DEFAULT_PORT


_MAC = re.compile(r"^[0-9a-fA-F]{12}$")


class ValidationError(ValueError):
    pass


def normalize_mac(value: str) -> str:
    compact = re.sub(r"[-:.\s]", "", value)
    if not _MAC.fullmatch(compact):
        raise ValidationError("Enter a valid 12-digit MAC address.")
    return ":".join(compact[index:index + 2].upper() for index in range(0, 12, 2))


def validate_port(value: object, *, default: bool = False) -> int:
    if value in (None, "") and default:
        return DEFAULT_PORT
    if isinstance(value, bool):
        raise ValidationError("Host port must be a number from 1 to 65535.")
    try:
        port = int(str(value), 10)
    except (TypeError, ValueError) as error:
        raise ValidationError("Host port must be a number from 1 to 65535.") from error
    if str(value).strip() != str(port) or not 1 <= port <= 65_535:
        raise ValidationError("Host port must be a number from 1 to 65535.")
    return port


def validate_address(value: object) -> str:
    address = str(value or "").strip()
    if not address or len(address) > 253 or any(character.isspace() for character in address):
        raise ValidationError("Enter a hostname or IP address.")
    try:
        parsed = ipaddress.ip_address(address)
    except ValueError:
        labels = address.removesuffix(".").split(".")
        if any(
            not 1 <= len(label) <= 63
            or not label[0].isalnum()
            or not label[-1].isalnum()
            or any(not (character.isascii() and (character.isalnum() or character == "-")) for character in label)
            for label in labels
        ):
            raise ValidationError("Enter a hostname or IPv4 address.")
        return address.removesuffix(".")
    if not isinstance(parsed, ipaddress.IPv4Address):
        raise ValidationError("Enter a hostname or IPv4 address.")
    return str(parsed)


def validate_broadcast(value: object) -> str | None:
    text = str(value or "").strip()
    if not text:
        return None
    try:
        return str(ipaddress.IPv4Address(text))
    except ipaddress.AddressValueError as error:
        raise ValidationError("Broadcast address must be a valid IPv4 address.") from error
