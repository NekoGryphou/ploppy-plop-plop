import asyncio
import io
import random
import unittest
from urllib.error import HTTPError
from unittest.mock import AsyncMock, patch

from decky_power.auth import AuthHeaders, canonical_message, response_signature, sign, verify_response
from decky_power.client import HostClient, HostError, HostReply
from decky_power.models import Device
from decky_power.protobuf import (
    PLUGIN_VERSION,
    DecodeError,
    ErrorResponse,
    StatusResponse,
    blob,
    decode_message,
    pair_request,
    shutdown_request,
    status_request,
    text,
    uint,
)
from decky_power.wol import magic_packet, send_magic_packet


class ProtocolTests(unittest.TestCase):
    def test_authenticated_requests_advertise_the_plugin_version(self) -> None:
        requests = (
            ("StatusRequest", status_request()),
            ("ShutdownRequest", shutdown_request()),
            ("PairRequest", pair_request(client_message=b"spake")),
        )
        for message_name, encoded in requests:
            with self.subTest(message=message_name):
                self.assertEqual(
                    decode_message(message_name, encoded)["client_version"],
                    PLUGIN_VERSION,
                )

    def test_status_decodes_and_skips_additive_field(self) -> None:
        encoded = text(1, "gaming-pc") + text(2, "1.2.0") + uint(3, 1) + uint(4, 1) + text(5, "host-id") + text(99, "future")
        response = StatusResponse.decode(encoded)
        self.assertEqual((response.hostname, response.host_version, response.protocol_version, response.paired, response.host_id), ("gaming-pc", "1.2.0", 1, True, "host-id"))

    def test_malformed_protobuf_fails(self) -> None:
        with self.assertRaises(DecodeError): StatusResponse.decode(blob(1, b"\xff"))

    def test_truncated_unknown_fixed_width_fields_fail(self) -> None:
        valid = text(1, "host") + text(2, "0.1.0") + uint(3, 1) + text(5, "id")
        for truncated_unknown in (b"\x31\x00\x00\x00", b"\x3d\x00\x00"):
            with self.subTest(field=truncated_unknown):
                with self.assertRaises(DecodeError):
                    StatusResponse.decode(valid + truncated_unknown)

    def test_known_field_with_wrong_wire_type_fails(self) -> None:
        with self.assertRaises(DecodeError):
            StatusResponse.decode(uint(1, 1))

    def test_structured_host_error_decodes(self) -> None:
        error = ErrorResponse.decode(uint(1, 2) + text(2, "The pairing code expired."))
        self.assertEqual((error.code, error.message), (2, "The pairing code expired."))

    def test_arbitrary_protobuf_input_never_escapes_codec_errors(self) -> None:
        generator = random.Random(0xDEC0DE)
        for _ in range(1_000):
            payload = generator.randbytes(generator.randrange(0, 129))
            try:
                StatusResponse.decode(payload)
            except DecodeError:
                pass

    def test_pairing_code_is_validated_in_the_backend(self) -> None:
        client = HostClient()
        device = Device(id="pc", name="PC", address="127.0.0.1", mac="AA:BB:CC:DD:EE:FF")
        for code in ("", "12345", "1234567", "１２３４５６", "abc123"):
            with self.subTest(code=code):
                with self.assertRaises(HostError): asyncio.run(client.pair(device, code))

    def test_authentication_canonicalization_covers_path_and_body(self) -> None:
        secret, nonce = bytes(range(32)), bytes(range(16))
        valid = sign(secret, "POST", "/v1/status", b"body", timestamp=100, nonce=nonce)
        changed_path = sign(secret, "POST", "/v1/shutdown", b"body", timestamp=100, nonce=nonce)
        changed_body = sign(secret, "POST", "/v1/status", b"changed", timestamp=100, nonce=nonce)
        self.assertNotEqual(valid.signature, changed_path.signature)
        self.assertNotEqual(valid.signature, changed_body.signature)
        self.assertTrue(canonical_message(100, nonce, "post", "/v1/status", b"").startswith(b"deckypower-auth-v1\0"))

    def test_response_authentication_rejects_missing_and_tampered_data(self) -> None:
        secret, nonce = bytes(range(32)), bytes(range(16))
        signature = response_signature(secret, nonce, "/v1/status", 200, b"body").hex()
        verify_response(secret, nonce, "/v1/status", 200, b"body", signature)
        for path, status, body, supplied in (
            ("/v1/shutdown", 200, b"body", signature),
            ("/v1/status", 202, b"body", signature),
            ("/v1/status", 200, b"changed", signature),
            ("/v1/status", 200, b"body", None),
        ):
            with self.subTest(path=path, status=status, body=body, supplied=supplied):
                with self.assertRaises(ValueError): verify_response(secret, nonce, path, status, body, supplied)

    def test_host_client_rejects_unsigned_authenticated_response(self) -> None:
        client = HostClient()
        device = Device(id="pc", name="PC", address="127.0.0.1", mac="AA:BB:CC:DD:EE:FF")
        with patch("decky_power.client.asyncio.to_thread", new=AsyncMock(return_value=HostReply(b"", 200, None))):
            with self.assertRaises(HostError):
                asyncio.run(client._post(device, "/v1/status", b"request", bytes(range(32))))

    def test_pairing_completion_retries_the_identical_body_once_after_transport_failure(self) -> None:
        client = HostClient()
        device = Device(id="pc", name="PC", address="127.0.0.1", mac="AA:BB:CC:DD:EE:FF")
        body = b"one immutable completion"
        failure = HostError("unavailable", "connection dropped")
        with patch.object(
            client,
            "_post",
            new=AsyncMock(side_effect=[failure, HostReply(b"response", 200)]),
        ) as post:
            reply = asyncio.run(client._complete_pairing(device, body))

        self.assertEqual(reply.body, b"response")
        self.assertEqual(post.await_count, 2)
        self.assertEqual(post.await_args_list[0].args, post.await_args_list[1].args)
        self.assertEqual(post.await_args_list[0].args, (device, "/v1/pair", body))

    def test_pairing_completion_does_not_retry_protocol_failures(self) -> None:
        client = HostClient()
        device = Device(id="pc", name="PC", address="127.0.0.1", mac="AA:BB:CC:DD:EE:FF")
        with patch.object(
            client,
            "_post",
            new=AsyncMock(side_effect=HostError("pairing", "rejected")),
        ) as post:
            with self.assertRaises(HostError):
                asyncio.run(client._complete_pairing(device, b"body"))
        self.assertEqual(post.await_count, 1)

    def test_host_client_authenticates_http_errors_before_trusting_them(self) -> None:
        client, secret, nonce = HostClient(), bytes(range(32)), bytes(range(16))
        device = Device(id="pc", name="PC", address="127.0.0.1", mac="AA:BB:CC:DD:EE:FF")
        body = uint(1, 5) + text(2, "Host update required.")

        def failure(signature: str | None) -> HTTPError:
            headers = {} if signature is None else {"X-Decky-Response-Signature": signature}
            return HTTPError("http://host/v1/status", 426, "Upgrade Required", headers, io.BytesIO(body))

        with patch("decky_power.client.sign", return_value=AuthHeaders(100, nonce, b"x" * 32)):
            with patch("decky_power.client.asyncio.to_thread", new=AsyncMock(side_effect=failure(None))):
                with self.assertRaises(HostError) as unsigned:
                    asyncio.run(client._post(device, "/v1/status", b"request", secret))
            self.assertEqual(unsigned.exception.kind, "integrity")

            signature = response_signature(secret, nonce, "/v1/status", 426, body).hex()
            with patch("decky_power.client.asyncio.to_thread", new=AsyncMock(side_effect=failure(signature))):
                with self.assertRaises(HostError) as authenticated:
                    asyncio.run(client._post(device, "/v1/status", b"request", secret))
            self.assertEqual((authenticated.exception.kind, authenticated.exception.status), ("protocol", 426))
            self.assertEqual(str(authenticated.exception), "Host update required.")

    def test_wol_packet_shape(self) -> None:
        packet = magic_packet("AA:BB:CC:DD:EE:FF")
        self.assertEqual(len(packet), 102)
        self.assertEqual(packet, b"\xff" * 6 + bytes.fromhex("AABBCCDDEEFF") * 16)

    def test_wol_uses_configured_broadcast_on_standard_ports(self) -> None:
        connection = unittest.mock.MagicMock()
        context = unittest.mock.MagicMock()
        context.__enter__.return_value = connection
        with patch("decky_power.wol.socket.socket", return_value=context):
            send_magic_packet("AA:BB:CC:DD:EE:FF", "192.168.1.255")
        packet = b"\xff" * 6 + bytes.fromhex("AABBCCDDEEFF") * 16
        connection.setsockopt.assert_called_once()
        self.assertEqual(connection.sendto.call_args_list, [
            unittest.mock.call(packet, ("192.168.1.255", 9)),
            unittest.mock.call(packet, ("192.168.1.255", 7)),
        ])


if __name__ == "__main__": unittest.main()
