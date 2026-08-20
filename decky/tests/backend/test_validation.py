import unittest

from decky_power.validation import ValidationError, normalize_mac, validate_port


class ValidationTests(unittest.TestCase):
    def test_common_mac_formats_normalize(self) -> None:
        for value in ("aa:bb:cc:dd:ee:ff", "AA-BB-CC-DD-EE-FF", "aabb.ccdd.eeff", "AABBCCDDEEFF"):
            self.assertEqual(normalize_mac(value), "AA:BB:CC:DD:EE:FF")

    def test_invalid_mac_fails(self) -> None:
        for value in ("", "AA:BB", "GG:BB:CC:DD:EE:FF"):
            with self.assertRaises(ValidationError): normalize_mac(value)

    def test_port_default_custom_boundaries_and_invalid_values(self) -> None:
        self.assertEqual(validate_port(None, default=True), 47991)
        for value in (1, "48100", 65535): self.assertEqual(validate_port(value), int(value))
        for value in (0, 65536, "x", "12.5", True):
            with self.assertRaises(ValidationError): validate_port(value)


if __name__ == "__main__": unittest.main()
