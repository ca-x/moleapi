"""Apply all frontend dependency patches against integrity-verified pristine npm files."""
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
web = Path('web').resolve()
lock = json.loads((web / 'package-lock.json').read_text())
with tempfile.TemporaryDirectory(prefix='moleapi-pristine-patches-') as temporary:
    root = Path(temporary)
    (root / 'package.json').write_text(json.dumps({'name': 'moleapi-localization-patch-verification', 'version': '0.0.0', 'private': True}))
    shutil.copytree(web / 'patches', root / 'patches')
    for package, version in packages.items():
        archive = Path(sys.argv[1]) / (package.replace('@', '').replace('/', '-') + '-' + version + '.tgz')
        expected = lock['packages']['node_modules/' + package]['integrity']
        actual = 'sha512-' + base64.b64encode(hashlib.sha512(archive.read_bytes()).digest()).decode()
        if actual != expected:
            raise ValueError('Tarball integrity mismatch: ' + package)
        destination = root / 'node_modules' / package
        destination.mkdir(parents=True)
        with tarfile.open(archive) as bundle:
            for member in bundle.getmembers():
                member.name = member.name.removeprefix('package/')
                if member.name != 'package':
                    bundle.extract(member, destination, filter='data')
    subprocess.run([str(web / 'node_modules/.bin/patch-package'), '--error-on-fail'], cwd=root, check=True)
