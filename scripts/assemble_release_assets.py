"""Stage the exact native release matrix and write its SHA256SUMS file."""

from __future__ import annotations

import hashlib
import shutil
import sys
from collections import Counter
from pathlib import Path

SERVER_TARGETS = {
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
}
EXPECTED = {".tar.gz": 5, ".deb": 1, ".rpm": 1, ".AppImage": 1, ".dmg": 2, ".exe": 1, ".msi": 2}


def kind(name: str) -> str | None:
    return next((suffix for suffix in EXPECTED if name.endswith(suffix)), None)


def assemble(source: Path, output: Path) -> list[Path]:
    if not source.is_dir():
        raise ValueError("Release artifact download directory is missing")
    if output.exists() and any(output.iterdir()):
        raise ValueError("Release staging directory must be empty")
    files = sorted(path for path in source.rglob("*") if path.is_file())
    if not files:
        raise ValueError("No release artifacts were downloaded")
    counts: Counter[str] = Counter()
    by_name: dict[str, Path] = {}
    for path in files:
        suffix = kind(path.name)
        if suffix is None:
            raise ValueError(f"Unexpected release artifact: {path.name}")
        if path.name in by_name:
            raise ValueError(f"Duplicate release artifact name: {path.name}")
        by_name[path.name] = path
        counts[suffix] += 1
    if dict(counts) != EXPECTED:
        raise ValueError(f"Incomplete release artifacts: {dict(counts)} != {EXPECTED}")
    servers = {f"moleapi-server-{target}.tar.gz" for target in SERVER_TARGETS}
    if {name for name in by_name if name.endswith(".tar.gz")} != servers:
        raise ValueError("Server target archives do not match the release matrix")
    output.mkdir(parents=True, exist_ok=True)
    staged = []
    for name, path in sorted(by_name.items()):
        target = output / name
        shutil.copyfile(path, target)
        staged.append(target)
    lines = []
    for path in staged:
        with path.open("rb") as stream:
            digest = hashlib.file_digest(stream, "sha256").hexdigest()
        lines.append(f"{digest}  {path.name}\n")
    (output / "SHA256SUMS").write_text("".join(lines), encoding="utf-8")
    return staged


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit("usage: assemble_release_assets.py DOWNLOAD_DIR STAGING_DIR")
    try:
        staged = assemble(Path(sys.argv[1]), Path(sys.argv[2]))
    except ValueError as error:
        raise SystemExit(str(error)) from error
    print(f"Staged {len(staged)} release assets and SHA256SUMS")
