import unittest

from decky_power.validation import ValidationError, normalize_mac, validate_address, validate_port


class ValidationTests(unittest.TestCase):
    def test_common_mac_formats_normalize(self) -> None:
        for value in ("AA:BB:CC:DD:EE:FF", "aa:bb:cc:dd:ee:ff", "AA-BB-CC-DD-EE-FF", "aabb.ccdd.eeff", "AABBCCDDEEFF", "  AA:BB:CC:DD:EE:FF  "):
            self.assertEqual(normalize_mac(value), "AA:BB:CC:DD:EE:FF")

    def test_invalid_mac_fails(self) -> None:
        for value in ("", "AA:BB", "AA:BB:CC:DD:EE", "AA:BB:CC:DD:EE:FF:00", "GG:BB:CC:DD:EE:FF"):
            with self.assertRaises(ValidationError): normalize_mac(value)

    def test_port_default_custom_boundaries_and_invalid_values(self) -> None:
        self.assertEqual(validate_port(None, default=True), 47991)
        for value in (1, "48100", 65535): self.assertEqual(validate_port(value), int(value))
        for value in (0, 65536, "x", "12.5", True):
            with self.assertRaises(ValidationError): validate_port(value)

    def test_address_accepts_only_ipv4_or_safe_dns_hostnames(self) -> None:
        for value, expected in (("gaming-pc.local", "gaming-pc.local"), ("PC.EXAMPLE.", "PC.EXAMPLE"), ("192.168.1.20", "192.168.1.20")):
            with self.subTest(value=value): self.assertEqual(validate_address(value), expected)
        for value in ("", "http://pc", "user@pc", "pc:123", "pc/path", "pc?query", "pc#fragment", "bad..local", "-bad.local", "bad-.local", "::1"):
            with self.subTest(value=value):
                with self.assertRaises(ValidationError): validate_address(value)


if __name__ == "__main__": unittest.main()
