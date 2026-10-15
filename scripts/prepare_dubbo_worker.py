#!/usr/bin/env python3
"""Build or stage the pinned Apache Dubbo worker for offline app packaging."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import uuid
import zipfile

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / 'vendor/dubbo-worker/manifest.json'
TARGET = ROOT / 'vendor/dubbo-worker/current'
POM = ROOT / 'tools/dubbo-worker/pom.xml'
BUILD_JAR = ROOT / 'tools/dubbo-worker/target/moleapi-dubbo-worker-0.1.0.jar'


def verify(jar: Path, manifest: dict):
    data = jar.read_bytes()
    if len(data) != manifest['worker_bytes'] or hashlib.sha256(data).hexdigest() != manifest['worker_sha256']:
        raise ValueError('Pinned Dubbo worker JAR size/SHA256 mismatch')
    with zipfile.ZipFile(jar) as source:
        names = source.namelist()
        if len(names) > 100_000 or len(names) != len(set(names)):
            raise ValueError('Dubbo worker JAR has duplicate/excessive entries')
        if any(PurePosixPath(name).is_absolute() or '..' in PurePosixPath(name).parts or '\\' in name
               for name in names):
            raise ValueError('Dubbo worker JAR has an unsafe entry')
        if 'app/moleapi/dubbo/Worker.class' not in names:
            raise ValueError('Dubbo worker main class is missing')
    return data


def prepare(jar: Path | None, maven: Path | None):
    manifest = json.loads(MANIFEST.read_text())
    if manifest['format'] != 'moleapi-dubbo-worker-v1' or manifest['dubbo_version'] != '3.3.6':
        raise ValueError('Unsupported Dubbo worker manifest')
    if TARGET.is_symlink():
        raise ValueError('Dubbo worker current resource cannot be a symlink')
    if jar is None and (TARGET / 'engine.jar').is_file():
        verify(TARGET / 'engine.jar', manifest)
        print(f'PASS existing pinned Apache Dubbo worker: {manifest["worker_sha256"]}')
        return
    if jar is None:
        executable = maven or Path(shutil.which('mvn') or '')
        if not executable.is_absolute() or not executable.is_file():
            raise ValueError('Choose an explicit absolute Maven executable to build Dubbo worker')
        subprocess.run([str(executable), '-B', '--strict-checksums', '-f', str(POM), 'package'],
                       cwd=ROOT, check=True, timeout=600)
        jar = BUILD_JAR
    elif not jar.is_absolute():
        raise ValueError('Supplied Dubbo worker JAR path must be absolute')
    data = verify(jar, manifest)
    TARGET.parent.mkdir(parents=True, exist_ok=True)
    ready = TARGET.parent / ('.dubbo-ready-' + uuid.uuid4().hex)
    backup = TARGET.parent / ('.dubbo-previous-' + uuid.uuid4().hex)
    try:
        ready.mkdir()
        (ready / 'engine.jar').write_bytes(data)
        (ready / 'moleapi-dubbo-worker.json').write_text(json.dumps({
            'format':manifest['format'], 'dubbo_version':manifest['dubbo_version'],
            'worker_sha256':manifest['worker_sha256'], 'worker_bytes':manifest['worker_bytes']
        }, indent=2) + '\n')
        if TARGET.exists(): TARGET.rename(backup)
        try: ready.rename(TARGET)
        except OSError:
            if backup.exists(): backup.rename(TARGET)
            raise
    finally:
        if ready.exists(): shutil.rmtree(ready)
        if backup.exists(): shutil.rmtree(backup)
    print(f'PASS pinned Apache Dubbo worker {manifest["dubbo_version"]}: {manifest["worker_sha256"]}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jar', type=Path, help='Already-built absolute JAR; exact hash is still required')
    parser.add_argument('--maven', type=Path, help='Absolute Maven executable when building source')
    args = parser.parse_args()
    prepare(args.jar, args.maven)
