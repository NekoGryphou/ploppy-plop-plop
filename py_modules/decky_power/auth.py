import hashlib
import hmac
import os
import struct
import time
from dataclasses import dataclass


DOMAIN = b"deckypower-auth-v1\0"


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
