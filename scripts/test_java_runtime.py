#!/usr/bin/env python3
"""Synthetic archive tests for build-time Java packaging; no target code executes."""
import hashlib
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile
import prepare_java_runtime as java


class Packaging(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / 'vendor/java-runtime').mkdir(parents=True)

    def archive(self, members=None, zip=False):
        members = members or [('jre/bin/java', b'native-runtime-file'), ('jre/legal/java.base/LICENSE', b'license fixture')]
        path = self.root / ('runtime.zip' if zip else 'runtime.tar.gz')
        if zip:
            with zipfile.ZipFile(path, 'w') as archive:
                for name, contents in members:
                    archive.writestr(name, contents)
        else:
            with tarfile.open(path, 'w:gz') as archive:
                for name, contents in members:
                    entry = tarfile.TarInfo(name)
                    if isinstance(contents, str):
                        entry.type = tarfile.SYMTYPE
                        entry.linkname = contents
                        archive.addfile(entry)
                    else:
                        entry.size = len(contents)
                        entry.mode = 0o755 if name.endswith('/java') else 0o644
                        archive.addfile(entry, io.BytesIO(contents))
        asset = {'url': 'https://fixtures.invalid/runtime.' + ('zip' if zip else 'tar.gz'),
                 'archive_root': 'jre', 'archive_bytes': path.stat().st_size,
                 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'java': 'payload/bin/java', 'legal': 'payload/legal'}
        manifest = {'format': 'moleapi-java-runtime-assets-v1', 'version': 'fixture', 'assets': {'fixture-target': asset}}
        (self.root / 'vendor/java-runtime/manifest.json').write_text(json.dumps(manifest))
        return path, asset

    def test_original_bytes_licenses_aliases_and_idempotent_preparation(self):
        path, _ = self.archive([('jre/bin/java', b'native-runtime-file'),
                                ('jre/legal/java.base/LICENSE', b'license fixture'),
                                ('jre/legal/other/LICENSE', '../java.base/LICENSE')])
        current = java.prepare(self.root, 'fixture-target', path)
        executable = current / 'payload/bin/java'
        self.assertEqual(executable.read_bytes(), b'native-runtime-file')
        self.assertEqual((current / 'payload/legal/other/LICENSE').read_bytes(), b'license fixture')
        self.assertFalse((current / 'payload/legal/other/LICENSE').is_symlink())
        first = executable.stat().st_mtime_ns
        java.prepare(self.root, 'fixture-target', path)
        self.assertEqual(executable.stat().st_mtime_ns, first)
        executable.write_bytes(b'corrupted cache')
        java.prepare(self.root, 'fixture-target', path)
        self.assertEqual(executable.read_bytes(), b'native-runtime-file')
        self.assertEqual(list((self.root / 'vendor/java-runtime').glob('.java-previous-*')), [])

    def test_hash_or_size_failure_retains_prior_payload(self):
        path, _ = self.archive()
        current = java.prepare(self.root, 'fixture-target', path)
        original = (current / 'files.sha256.json').read_bytes()
        value = bytearray(path.read_bytes())
        value[-1] ^= 1
        path.write_bytes(value)
        with self.assertRaisesRegex(ValueError, 'SHA256'):
            java.prepare(self.root, 'fixture-target', path)
        self.assertEqual((current / 'files.sha256.json').read_bytes(), original)
        path.write_bytes(b'truncated')
        with self.assertRaisesRegex(ValueError, 'size'):
            java.prepare(self.root, 'fixture-target', path)
        self.assertEqual((current / 'files.sha256.json').read_bytes(), original)

    def test_zip_preserves_bytes_and_rejects_escaping_or_ambiguous_paths(self):
        path, _ = self.archive(zip=True)
        java.prepare(self.root, 'fixture-target', path)
        for name in ('jre/../outside', '/jre/bin/java', 'jre/bin\\java', 'jre/C:/java'):
            with self.subTest(name=name):
                path, _ = self.archive([(name, b'bad')], zip=True)
                with self.assertRaises(ValueError):
                    java.prepare(self.root, 'fixture-target', path)
        path, _ = self.archive([('jre/bin/java', b'a'), ('jre/BIN/java', b'b')], zip=True)
        with self.assertRaisesRegex(ValueError, 'Duplicate'):
            java.prepare(self.root, 'fixture-target', path)
        self.assertFalse((self.root / 'outside').exists())

    def test_tar_rejects_escape_links_and_missing_license(self):
        path, _ = self.archive([('jre/bin/java', '../../outside')])
        with self.assertRaises((ValueError, tarfile.FilterError)):
            java.prepare(self.root, 'fixture-target', path)
        path, _ = self.archive([('jre/bin/java', b'bytes')])
        with self.assertRaisesRegex(ValueError, 'legal'):
            java.prepare(self.root, 'fixture-target', path)
        self.assertFalse((self.root / 'vendor/java-runtime/current').exists())

    def test_limits_and_unknown_target_fail_without_activation(self):
        path, _ = self.archive()
        with patch.object(java, 'PAYLOAD_LIMIT', 4), self.assertRaisesRegex(ValueError, 'limit'):
            java.prepare(self.root, 'fixture-target', path)
        with self.assertRaisesRegex(ValueError, 'target'):
            java.prepare(self.root, 'not-the-build-target', path)
        self.assertFalse((self.root / 'vendor/java-runtime/current').exists())


if __name__ == '__main__':
    unittest.main()
