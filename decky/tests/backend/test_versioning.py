import unittest

from decky_my_rig.versioning import VersionRelation, compare_versions


class VersioningTests(unittest.TestCase):
    def test_patch_differences_are_compatible(self) -> None:
        self.assertEqual(compare_versions("1.2.9", "1.2.0"), VersionRelation.COMPATIBLE)

    def test_minor_difference_identifies_update_direction(self) -> None:
        self.assertEqual(compare_versions("1.3.0", "1.2.9"), VersionRelation.UPDATE_HOST)
        self.assertEqual(compare_versions("1.2.0", "1.3.0"), VersionRelation.UPDATE_PLUGIN)

    def test_major_and_malformed_versions_are_explicit(self) -> None:
        self.assertEqual(compare_versions("2.0.0", "1.9.0"), VersionRelation.INCOMPATIBLE)
        self.assertEqual(compare_versions("v1.2.3", "1.2.3"), VersionRelation.UNKNOWN)


if __name__ == "__main__":
    unittest.main()
