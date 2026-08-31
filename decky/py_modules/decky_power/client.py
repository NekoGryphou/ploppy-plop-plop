from __future__ import annotations

import asyncio
import hmac
from dataclasses import dataclass
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

from cryptography.hazmat.primitives import hashes
from cryptography.exceptions import InvalidTag
from cryptography.hazmat.primitives.ciphers.aead import ChaCha20Poly1305
from cryptography.hazmat.primitives.kdf.hkdf import HKDF
from spake2 import SPAKE2_A

from .auth import sign, verify_response
from .models import Device
from .protobuf import PROTOCOL_VERSION, ErrorResponse, PairResponse, StatusResponse, pair_request, pairing_credential_aad, shutdown_request, status_request


class HostError(RuntimeError):
    def __init__(self, kind: str, message: str, status: int | None = None):
        super().__init__(message)
        self.kind = kind
        self.status = status


@dataclass(frozen=True)
class HostReply:
    body: bytes
    status: int
    response_signature: str | None = None


class HostClient:
    def __init__(self, timeout: float = 4.0):
        self.timeout = timeout

    async def pair(self, device: Device, pairing_code: str) -> tuple[bytes, PairResponse]:
        pairing_code = "".join(pairing_code.split())
        if len(pairing_code) != 6 or not pairing_code.isascii() or not pairing_code.isdigit():
            raise HostError("pairing", "Enter the six-digit code shown by DeckyPowerHostControl.")
        exchange = SPAKE2_A(
            pairing_code.encode("ascii"),
            idA=b"decky-client",
            idB=b"decky-host",
        )
        client_message = exchange.start()
        started_reply = await self._post(
            device, "/v1/pair", pair_request(client_message)
        )
        started = PairResponse.decode(started_reply.body)
        if started.protocol_version != PROTOCOL_VERSION:
            raise HostError("protocol", "DeckyPowerHost needs to be updated.")
        shared = exchange.finish(started.host_message)
        confirmation = hmac.digest(
            shared,
            b"deckypower-pairing-confirm-v1\0"
            + client_message
            + started.host_message
            + started.session_id,
            "sha256",
        )
        completion_body = pair_request(
            session_id=started.session_id,
            confirmation=confirmation,
        )
        completed_reply = await self._complete_pairing(device, completion_body)
        response = PairResponse.decode(completed_reply.body)
        if response.protocol_version != PROTOCOL_VERSION:
            raise HostError("protocol", "DeckyPowerHost needs to be updated.")
        key = HKDF(
            algorithm=hashes.SHA256(),
            length=32,
            salt=None,
            info=b"deckypower-pairing-credential-v1",
        ).derive(shared)
        try:
            credential = ChaCha20Poly1305(key).decrypt(
                response.nonce,
                response.ciphertext,
                pairing_credential_aad(response),
            )
        except (InvalidTag, ValueError) as error:
            raise HostError(
                "pairing", "Pairing could not be authenticated."
            ) from error
        if len(credential) != 32:
            raise HostError("pairing", "The host returned an invalid credential.")
        return credential, response

    async def _complete_pairing(self, device: Device, body: bytes) -> HostReply:
        try:
            return await self._post(device, "/v1/pair", body)
        except HostError as error:
            if error.kind != "unavailable":
                raise
            return await self._post(device, "/v1/pair", body)

    async def status(self, device: Device, credential: bytes) -> StatusResponse:
        reply = await self._post(device, "/v1/status", status_request(), credential)
        response = StatusResponse.decode(reply.body)
        if response.protocol_version != PROTOCOL_VERSION:
            raise HostError("protocol", "DeckyPowerHost needs to be updated.")
        if device.host_id and response.host_id != device.host_id:
            raise HostError("identity", "This address now belongs to a different PC.")
        return response

    async def probe(self, device: Device) -> None:
        try:
            await self._post(device, "/v1/status", status_request())
        except HostError as error:
            if error.kind == "authentication": return
            raise
        raise HostError("protocol", "DeckyPowerHost accepted an unauthenticated status request.")

    async def shutdown(self, device: Device, credential: bytes) -> None:
        await self._post(device, "/v1/shutdown", shutdown_request(), credential)

    async def _post(
        self,
        device: Device,
        path: str,
        body: bytes,
        credential: bytes | None = None,
    ) -> HostReply:
        headers = {"Content-Type": "application/x-protobuf", "Accept": "application/x-protobuf"}
        authentication = sign(credential, "POST", path, body) if credential is not None else None
        if authentication is not None:
            headers.update(authentication.as_http())
        request = Request(
            f"http://{device.address}:{device.port}{path}",
            data=body,
            headers=headers,
            method="POST",
        )
        try:
            reply = await asyncio.to_thread(self._open, request)
            self._verify_reply(reply, credential, authentication, path)
            return reply
        except HTTPError as error:
            try:
                error_body = error.read(64 * 1024)
                response_signature = (
                    error.headers.get("X-Decky-Response-Signature")
                    if error.headers is not None
                    else None
                )
                reply = HostReply(error_body, error.code, response_signature)
                self._verify_reply(reply, credential, authentication, path)
                message, kind = self._decode_error(error.code, error_body)
            finally:
                error.close()
            raise HostError(kind, message, error.code) from error
        except (URLError, TimeoutError, OSError) as error:
            raise HostError(
                "unavailable",
                f"DeckyPowerHost could not be reached at {device.address}:{device.port}.",
            ) from error

    @staticmethod
    def _verify_reply(reply, credential, authentication, path: str) -> None:
        if credential is None or authentication is None:
            return
        try:
            verify_response(
                credential,
                authentication.nonce,
                path,
                reply.status,
                reply.body,
                reply.response_signature,
            )
        except ValueError as error:
            raise HostError(
                "integrity",
                "DeckyPowerHost returned a response that could not be authenticated.",
            ) from error

    @staticmethod
    def _decode_error(status: int, body: bytes) -> tuple[str, str]:
        try:
            message = ErrorResponse.decode(body).message
        except ValueError:
            message = "DeckyPowerHost rejected the request."
        if status in (401, 409):
            kind = "authentication"
        elif status == 410:
            kind = "pairing"
        elif status == 426:
            kind = "protocol"
        else:
            kind = "host"
        return message, kind

    def _open(self, request: Request) -> HostReply:
        with urlopen(request, timeout=self.timeout) as response:
            return HostReply(
                response.read(64 * 1024),
                response.status,
                response.headers.get("X-Decky-Response-Signature"),
            )
