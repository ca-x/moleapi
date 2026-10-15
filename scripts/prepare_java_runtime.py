#!/usr/bin/env python3
"""Prepare the pinned build-time JRE; product runtime never downloads Java."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import platform
import shutil
import stat
import tarfile
import tempfile
import urllib.request
import uuid
import zipfile

ROOT = Path(__file__).resolve().parents[1]
ARCHIVE_LIMIT = 96 * 1024 * 1024
PAYLOAD_LIMIT = 512 * 1024 * 1024
FILE_LIMIT = 10000
RECEIPT_FORMAT = 'moleapi-java-runtime-v1'


def host_target():
    key = (platform.system(), platform.machine().lower())
    targets = {('Linux', 'x86_64'): 'x86_64-unknown-linux-gnu',
               ('Linux', 'aarch64'): 'aarch64-unknown-linux-gnu',
               ('Darwin', 'x86_64'): 'x86_64-apple-darwin',
               ('Darwin', 'arm64'): 'aarch64-apple-darwin',
               ('Windows', 'amd64'): 'x86_64-pc-windows-msvc'}
    if key not in targets:
        raise ValueError('Unsupported Java build platform; select an explicit supported target')
    if key[0] == 'Linux' and platform.libc_ver()[0] != 'glibc':
        raise ValueError('The pinned Linux runtime requires glibc, not musl')
    return targets[key]


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def verify_archive(path, asset):
    if path.stat().st_size != asset['archive_bytes'] or path.stat().st_size > ARCHIVE_LIMIT:
        raise ValueError('Java archive size mismatch')
    if digest(path) != asset['sha256']:
        raise ValueError('Java archive SHA256 mismatch')


def fetch_archive(asset, cache):
    cache.mkdir(parents=True, exist_ok=True)
    suffix = '.zip' if asset['url'].endswith('.zip') else '.tar.gz'
    cached = cache / (asset['sha256'] + suffix)
    if cached.is_file():
        verify_archive(cached, asset)
        return cached
    with tempfile.NamedTemporaryFile(dir=cache, delete=False) as temporary:
        candidate = Path(temporary.name)
        try:
            with urllib.request.urlopen(asset['url'], timeout=60) as response:
                size = 0
                while chunk := response.read(1024 * 1024):
                    size += len(chunk)
                    if size > asset['archive_bytes'] or size > ARCHIVE_LIMIT:
                        raise ValueError('Java download exceeded size limit')
                    temporary.write(chunk)
            temporary.flush()
            verify_archive(candidate, asset)
            temporary.close()
            candidate.replace(cached)
        finally:
            candidate.unlink(missing_ok=True)
    return cached


def member_path(name, expected_root):
    path = PurePosixPath(name)
    if path.is_absolute() or not path.parts or path.parts[0] != expected_root:
        raise ValueError('Unexpected Java archive root')
    if '\\' in name or ':' in name or any(part in ('.', '..') for part in name.split('/')):
        raise ValueError('Unsafe Java archive path')
    return path


def extract_archive(archive, destination, asset):
    expected_root = asset['archive_root']
    total = 0
    seen = set()
    def accept(name, size):
        nonlocal total
        path = member_path(name.rstrip('/'), expected_root)
        key = str(path).casefold()
        if key in seen:
            raise ValueError('Duplicate Java archive path')
        seen.add(key)
        total += size
        if len(seen) > FILE_LIMIT or total > PAYLOAD_LIMIT:
            raise ValueError('Java archive extraction limit exceeded')
        return path
    if asset['url'].endswith('.zip'):
        with zipfile.ZipFile(archive) as source:
            for member in source.infolist():
                path = accept(member.filename, member.file_size)
                mode = member.external_attr >> 16
                if stat.S_ISLNK(mode) or (stat.S_IFMT(mode) not in (0, stat.S_IFREG, stat.S_IFDIR)):
                    raise ValueError('Unsupported Java ZIP member')
                target = destination.joinpath(*path.parts)
                if member.is_dir():
                    target.mkdir(parents=True, exist_ok=True)
                else:
                    target.parent.mkdir(parents=True, exist_ok=True)
                    with source.open(member) as data, target.open('xb') as output:
                        shutil.copyfileobj(data, output, length=1024 * 1024)
                    if mode & 0o777:
                        target.chmod(mode & 0o777)
    else:
        if not hasattr(tarfile, 'data_filter'):
            raise ValueError('Use Python3.12+ with the standard tarfile data filter')
        with tarfile.open(archive, 'r:gz') as source:
            members = source.getmembers()
            for member in members:
                accept(member.name, member.size)
                if not (member.isfile() or member.isdir() or member.issym() or member.islnk()):
                    raise ValueError('Unsupported Java TAR member')
            source.extractall(destination, members=members, filter='data')
    extracted = destination / expected_root
    for path in extracted.rglob('*'):
        if path.is_symlink() and not path.resolve().is_relative_to(extracted.resolve()):
            raise ValueError('Java archive link escapes distribution')
    return extracted


def receipt(manifest, target):
    return {'format': RECEIPT_FORMAT, 'version': manifest['version'], 'target': target,
            'archive_sha256': manifest['assets'][target]['sha256']}


def inventory(root):
    return {path.relative_to(root).as_posix(): digest(path)
            for path in sorted((root / 'payload').rglob('*')) if path.is_file()}


def verify_payload(root, manifest, target):
    asset = manifest['assets'][target]
    if json.loads((root / 'moleapi-java-runtime.json').read_text()) != receipt(manifest, target):
        raise ValueError('Java runtime receipt mismatch')
    expected = json.loads((root / 'files.sha256.json').read_text())
    if not expected or inventory(root) != expected:
        raise ValueError('Java runtime inventory mismatch')
    if not (root / asset['java']).is_file() or not (root / asset['legal']).is_dir():
        raise ValueError('Java executable or legal directory missing')
    if not any((root / asset['legal']).rglob('LICENSE')):
        raise ValueError('Java distribution license missing')


def prepare(repository, target, archive=None):
    manifest = json.loads((repository / 'vendor/java-runtime/manifest.json').read_text())
    if manifest['format'] != 'moleapi-java-runtime-assets-v1' or target not in manifest['assets']:
        raise ValueError('Unsupported pinned Java runtime target')
    asset = manifest['assets'][target]
    base = repository / 'vendor/java-runtime'
    current = base / 'current'
    source = Path(archive) if archive else fetch_archive(asset, base / 'downloads')
    verify_archive(source, asset)
    try:
        verify_payload(current, manifest, target)
    except (OSError, ValueError, KeyError):
        with tempfile.TemporaryDirectory(prefix='.java-prepare-', dir=base) as temporary:
            stage = Path(temporary)
            extracted = extract_archive(source, stage / 'extracted', asset)
            ready = stage / 'ready'
            ready.mkdir()
            # Tauri resource copying materializes aliases too. Keep all original
            # file bytes and legal notices with the same portable regular-file layout.
            shutil.copytree(extracted, ready / 'payload', symlinks=False)
            (ready / 'moleapi-java-runtime.json').write_text(json.dumps(receipt(manifest, target), indent=2) + '\n')
            (ready / 'files.sha256.json').write_text(json.dumps(inventory(ready), indent=2) + '\n')
            verify_payload(ready, manifest, target)
            backup = base / ('.java-previous-' + uuid.uuid4().hex)
            if current.exists():
                current.rename(backup)
            try:
                ready.rename(current)
            except OSError:
                if backup.exists():
                    backup.rename(current)
                raise
            if backup.exists():
                shutil.rmtree(backup)
    if 'corresponding_source' in manifest:
        (current / 'corresponding-source.json').write_text(json.dumps(manifest['corresponding_source'], indent=2) + '\n')
    config = repository / 'desktop/tauri.java.json'
    config.parent.mkdir(parents=True, exist_ok=True)
    config.write_text(json.dumps({'bundle': {'resources': {'../vendor/java-runtime/current/': 'runtime/java/'}}}, indent=2) + '\n')
    print(f'PASS pinned Temurin{manifest["version"]}: {target}; original runtime files and legal notices verified')
    return current


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target', help='Explicit release target, mandatory for cross builds')
    parser.add_argument('--archive', type=Path, help='Existing exact pinned archive; still hash verified')
    args = parser.parse_args()
    prepare(ROOT, args.target or host_target(), args.archive)
