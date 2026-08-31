import hashlib
import hmac
import os
import struct
import time
from dataclasses import dataclass


DOMAIN = b"deckypower-auth-v1\0"
RESPONSE_DOMAIN = b"deckypower-response-v1\0"


def canonical_message(timestamp: int, nonce: bytes, method: str, path: str, body: bytes) -> bytes:
    if len(nonce) != 16: raise ValueError("nonce must be 16 bytes")
    fields = (nonce, method.upper().encode("ascii"), path.encode("ascii"))
    return DOMAIN + struct.pack(">Q", timestamp) + b"".join(struct.pack(">H", len(field)) + field for field in fields) + hashlib.sha256(body).digest()


@dataclass(frozen=True)
class AuthHeaders:
    timestamp: int
    nonce: bytes
    signature: bytes

    def as_http(self) -> dict[str, str]:
        return {"X-Decky-Timestamp": str(self.timestamp), "X-Decky-Nonce": self.nonce.hex(), "X-Decky-Signature": self.signature.hex()}


def sign(secret: bytes, method: str, path: str, body: bytes, *, timestamp: int | None = None, nonce: bytes | None = None) -> AuthHeaders:
    timestamp = int(time.time()) if timestamp is None else timestamp
    nonce = os.urandom(16) if nonce is None else nonce
    signature = hmac.digest(secret, canonical_message(timestamp, nonce, method, path, body), "sha256")
    return AuthHeaders(timestamp, nonce, signature)


def response_signature(secret: bytes, request_nonce: bytes, path: str, status: int, body: bytes) -> bytes:
    if len(request_nonce) != 16 or not 100 <= status <= 999:
        raise ValueError("invalid response authentication context")
    path_bytes = path.encode("ascii")
    message = RESPONSE_DOMAIN + struct.pack(">H", len(request_nonce)) + request_nonce + struct.pack(">H", len(path_bytes)) + path_bytes + struct.pack(">H", status) + hashlib.sha256(body).digest()
    return hmac.digest(secret, message, "sha256")


def verify_response(secret: bytes, request_nonce: bytes, path: str, status: int, body: bytes, signature: str | None) -> None:
    try:
        supplied = bytes.fromhex(signature or "")
    except ValueError as error:
        raise ValueError("host response authentication is malformed") from error
    if not hmac.compare_digest(response_signature(secret, request_nonce, path, status, body), supplied):
        raise ValueError("host response authentication failed")
