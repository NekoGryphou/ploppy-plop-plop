from __future__ import annotations

import asyncio
import hmac
from dataclasses import dataclass
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.ciphers.aead import ChaCha20Poly1305
from cryptography.hazmat.primitives.kdf.hkdf import HKDF
from spake2 import SPAKE2_A

from .auth import sign
from .models import Device
from .protobuf import PairResponse, StatusResponse, pair_request, shutdown_request, status_request


class HostError(RuntimeError):
    def __init__(self, kind: str, message: str, status: int | None = None):
        super().__init__(message); self.kind, self.status = kind, status


@dataclass(frozen=True)
class HostReply:
    body: bytes
    status: int


class HostClient:
    def __init__(self, timeout: float = 4.0): self.timeout = timeout

    async def pair(self, device: Device, pairing_code: str) -> tuple[bytes, PairResponse]:
        exchange = SPAKE2_A(pairing_code.encode("ascii"), idA=b"decky-client", idB=b"decky-host")
        client_message = exchange.start(); started = PairResponse.decode((await self._post(device, "/v1/pair", pair_request(client_message))).body)
        if started.protocol_version != 1: raise HostError("protocol", "DeckyPowerHost needs to be updated.")
        shared = exchange.finish(started.host_message)
        confirmation = hmac.digest(shared, b"deckypower-pairing-confirm-v1\0" + client_message + started.host_message + started.session_id, "sha256")
        response = PairResponse.decode((await self._post(device, "/v1/pair", pair_request(session_id=started.session_id, confirmation=confirmation))).body)
        key = HKDF(algorithm=hashes.SHA256(), length=32, salt=None, info=b"deckypower-pairing-credential-v1").derive(shared)
        try: credential = ChaCha20Poly1305(key).decrypt(response.nonce, response.ciphertext, started.host_message)
        except Exception as error: raise HostError("pairing", "Pairing could not be authenticated.") from error
        if len(credential) != 32: raise HostError("pairing", "The host returned an invalid credential.")
        return credential, response

    async def status(self, device: Device, credential: bytes) -> StatusResponse:
        response = StatusResponse.decode((await self._post(device, "/v1/status", status_request(), credential)).body)
        if response.protocol_version != 1: raise HostError("protocol", "DeckyPowerHost needs to be updated.")
        if device.host_id and response.host_id != device.host_id: raise HostError("identity", "This address now belongs to a different PC.")
        return response

    async def shutdown(self, device: Device, credential: bytes) -> None:
        await self._post(device, "/v1/shutdown", shutdown_request(), credential)

    async def _post(self, device: Device, path: str, body: bytes, credential: bytes | None = None) -> HostReply:
        headers = {"Content-Type": "application/x-protobuf", "Accept": "application/x-protobuf"}
        if credential is not None: headers.update(sign(credential, "POST", path, body).as_http())
        request = Request(f"http://{device.address}:{device.port}{path}", data=body, headers=headers, method="POST")
        try: return await asyncio.to_thread(self._open, request)
        except HTTPError as error:
            kind = "authentication" if error.code in (401, 409) else "protocol" if error.code == 426 else "host"
            raise HostError(kind, "DeckyPowerHost rejected the request.", error.code) from error
        except (URLError, TimeoutError, OSError) as error: raise HostError("unavailable", f"DeckyPowerHost could not be reached at {device.address}:{device.port}.") from error

    def _open(self, request: Request) -> HostReply:
        with urlopen(request, timeout=self.timeout) as response: return HostReply(response.read(64 * 1024), response.status)
