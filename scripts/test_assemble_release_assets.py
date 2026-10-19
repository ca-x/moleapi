import hashlib
import tempfile
import unittest
from pathlib import Path

from assemble_release_assets import SERVER_TARGETS, assemble


class ReleaseAssetsTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.source = Path(self.temp.name) / "downloaded"
        self.output = Path(self.temp.name) / "staged"
        server = self.source / "server"
        desktop = self.source / "desktop"
        server.mkdir(parents=True)
        desktop.mkdir()
        for target in SERVER_TARGETS:
            (server / f"moleapi-server-{target}.tar.gz").write_bytes(target.encode())
        for name in ("linux.deb", "linux.rpm", "linux.AppImage", "mac-intel.dmg", "mac-arm.dmg", "windows.exe", "windows-en-US.msi", "windows-zh-CN.msi"):
            (desktop / name).write_bytes(name.encode())

    def test_stages_complete_matrix_with_matching_checksums(self):
        staged = assemble(self.source, self.output)
        self.assertEqual(len(staged), 13)
        lines = (self.output / "SHA256SUMS").read_text().splitlines()
        self.assertEqual(len(lines), 13)
        for path in staged:
            self.assertIn(f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}", lines)

    def test_missing_archive_rejects_before_writing(self):
        next((self.source / "server").glob("*.tar.gz")).unlink()
        with self.assertRaisesRegex(ValueError, "Incomplete release artifacts"):
            assemble(self.source, self.output)
        self.assertFalse(self.output.exists())

    def test_duplicate_name_rejects_before_writing(self):
        duplicate = self.source / "desktop" / "nested"
        duplicate.mkdir()
        (duplicate / "linux.deb").write_bytes(b"different")
        with self.assertRaisesRegex(ValueError, "Duplicate release artifact name"):
            assemble(self.source, self.output)
        self.assertFalse(self.output.exists())


if __name__ == "__main__":
    unittest.main()
