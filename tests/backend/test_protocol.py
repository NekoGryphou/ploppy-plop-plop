import unittest

from decky_power.auth import canonical_message, sign
from decky_power.protobuf import DecodeError, StatusResponse, blob, text, uint
from decky_power.wol import magic_packet


class ProtocolTests(unittest.TestCase):
    def test_status_decodes_and_skips_additive_field(self) -> None:
        encoded = text(1, "gaming-pc") + text(2, "1.2.0") + uint(3, 1) + uint(4, 1) + text(5, "host-id") + text(99, "future")
        response = StatusResponse.decode(encoded)
        self.assertEqual((response.hostname, response.host_version, response.protocol_version, response.paired, response.host_id), ("gaming-pc", "1.2.0", 1, True, "host-id"))

    def test_malformed_protobuf_fails(self) -> None:
        with self.assertRaises(DecodeError): StatusResponse.decode(blob(1, b"\xff"))

    def test_authentication_canonicalization_covers_path_and_body(self) -> None:
        secret, nonce = bytes(range(32)), bytes(range(16))
        valid = sign(secret, "POST", "/v1/status", b"body", timestamp=100, nonce=nonce)
        changed_path = sign(secret, "POST", "/v1/shutdown", b"body", timestamp=100, nonce=nonce)
        changed_body = sign(secret, "POST", "/v1/status", b"changed", timestamp=100, nonce=nonce)
        self.assertNotEqual(valid.signature, changed_path.signature)
        self.assertNotEqual(valid.signature, changed_body.signature)
        self.assertTrue(canonical_message(100, nonce, "post", "/v1/status", b"").startswith(b"deckypower-auth-v1\0"))

    def test_wol_packet_shape(self) -> None:
        packet = magic_packet("AA:BB:CC:DD:EE:FF")
        self.assertEqual(len(packet), 102)
        self.assertEqual(packet[:6], b"\xff" * 6)
        self.assertEqual(packet[6:12], bytes.fromhex("AABBCCDDEEFF"))


if __name__ == "__main__": unittest.main()
