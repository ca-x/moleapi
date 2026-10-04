"""Capture standard patch-package diffs from verified cached npm tarballs.

Used when npm 12's allow-remote policy blocks patch-package's temporary install.
Usage: python3 tools/localization/capture-patches.py /path/to/npm-pack-tarballs
"""
import base64
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
packages = {'graphiql': '5.4.0', '@graphiql/react': '0.39.1', '@graphiql/plugin-doc-explorer': '0.4.4', '@graphiql/plugin-history': '0.4.4', 'monaco-editor': '0.52.2', '@codemirror/search': '6.7.2'}
lock = json.loads(Path('web/package-lock.json').read_text())
for package, version in packages.items():
    archive = Path(sys.argv[1]) / (package.replace('@', '').replace('/', '-') + '-' + version + '.tgz')
    expected = lock['packages']['node_modules/' + package]['integrity']
    actual = 'sha512-' + base64.b64encode(hashlib.sha512(archive.read_bytes()).digest()).decode()
    if actual != expected:
        raise ValueError('Tarball integrity mismatch: ' + package)
    with tempfile.TemporaryDirectory(prefix='moleapi-patch-') as temporary:
        root = Path(temporary)
        pristine = root / 'a/node_modules' / package
        current = root / 'b/node_modules' / package
        pristine.mkdir(parents=True)
        with tarfile.open(archive) as bundle:
            for member in bundle.getmembers():
                member.name = member.name.removeprefix('package/')
                if member.name == 'package':
                    continue
                bundle.extract(member, pristine, filter='data')
        shutil.copytree(Path('web/node_modules') / package, current, ignore=shutil.ignore_patterns('node_modules'))
        result = subprocess.run(['git', 'diff', '--no-index', '--', 'a/node_modules/' + package, 'b/node_modules/' + package], cwd=root, capture_output=True, text=True)
        if result.returncode not in (0, 1):
            raise RuntimeError(result.stderr)
        diff = result.stdout.replace('a/a/node_modules/', 'a/node_modules/').replace('b/b/node_modules/', 'b/node_modules/').replace('a/b/node_modules/', 'a/node_modules/')
        patch = Path('web/patches') / (package.replace('/', '+') + '+' + version + '.patch')
        patch.write_text(diff)
        print(str(patch), len(diff.splitlines()), 'lines')
