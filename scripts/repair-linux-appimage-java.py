#!/usr/bin/env python3
"""Restore the exact pinned JRE after linuxdeploy mutates AppImage ELF resources."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
from urllib.request import Request, urlopen

from prepare_java_runtime import verify_payload

ROOT = Path(__file__).resolve().parents[1]
TARGET = "x86_64-unknown-linux-gnu"
PLUGIN_URL = ("https://github.com/linuxdeploy/linuxdeploy-plugin-appimage/releases/"
              "download/continuous/linuxdeploy-plugin-appimage-x86_64.AppImage")
PLUGIN_SHA256 = "49d6a17160675a6bd1781699aae6bdf7692d98552e02a3671d2183d10547842e"
MAX_PLUGIN = 32 * 1024 * 1024
MAX_RUNTIME = 2 * 1024 * 1024


def checked_plugin(path: Path | None, temporary: Path) -> Path:
    if path is None:
        path = temporary / "linuxdeploy-plugin-appimage-x86_64.AppImage"
        with urlopen(Request(PLUGIN_URL, headers={"User-Agent": "moleapi-build"}), timeout=60) as response:
            total = 0
            with path.open("wb") as output:
                while chunk := response.read(1024 * 1024):
                    total += len(chunk)
                    if total > MAX_PLUGIN:
                        raise ValueError("AppImage packaging plugin exceeds size limit")
                    output.write(chunk)
    if hashlib.sha256(path.read_bytes()).hexdigest() != PLUGIN_SHA256:
        raise ValueError("AppImage packaging plugin hash mismatch")
    path.chmod(path.stat().st_mode | 0o100)
    return path


def extract(image: Path, destination: Path) -> Path:
    destination.mkdir()
    subprocess.run([str(image), "--appimage-extract"], cwd=destination,
                   env={**os.environ, "APPIMAGE_EXTRACT_AND_RUN": "1"},
                   stdout=subprocess.DEVNULL, check=True, timeout=180)
    appdir = destination / "squashfs-root"
    if not appdir.is_dir():
        raise ValueError("AppImage extraction did not create an AppDir")
    return appdir


def verify_java(resources: Path) -> None:
    manifest = json.loads((ROOT / "vendor/java-runtime/manifest.json").read_text())
    verify_payload(resources / "runtime/java", manifest, TARGET)


def repair(bundle_directory: Path, source_java: Path, plugin: Path | None) -> None:
    images = list(bundle_directory.resolve(strict=True).rglob("*.AppImage"))
    if len(images) != 1:
        raise ValueError(f"Expected one Linux AppImage, found {len(images)}")
    image = images[0]
    if image.with_suffix(image.suffix + ".sig").exists():
        raise ValueError("Refusing to alter an already signed AppImage")
    runtime_env = {key: value for key, value in os.environ.items()
                   if key != "APPIMAGE_EXTRACT_AND_RUN"}
    signature = subprocess.run([str(image), "--appimage-signature"],
                               capture_output=True, check=True, timeout=15, env=runtime_env)
    if signature.stdout.strip():
        raise ValueError("Refusing to alter an AppImage with an embedded signature")
    source_java = source_java.resolve(strict=True)
    manifest = json.loads((ROOT / "vendor/java-runtime/manifest.json").read_text())
    verify_payload(source_java, manifest, TARGET)
    with tempfile.TemporaryDirectory(prefix="moleapi-appimage-repair-", dir=image.parent) as temporary:
        base = Path(temporary)
        appdir = extract(image, base / "original")
        resources = appdir / "usr/lib/MoleAPI"
        installed_java = resources / "runtime/java"
        if not resources.is_dir() or not installed_java.is_dir():
            raise ValueError("AppImage bundled Java directory missing")
        if not installed_java.samefile(source_java):
            shutil.rmtree(installed_java)
            shutil.copytree(source_java, installed_java, symlinks=True)
        verify_java(resources)

        offset = int(subprocess.check_output([str(image), "--appimage-offset"],
                                             text=True, timeout=15, env=runtime_env).strip())
        if not 0 < offset <= MAX_RUNTIME:
            raise ValueError("AppImage runtime offset exceeds limit")
        runtime = base / "runtime-x86_64"
        with image.open("rb") as original, runtime.open("wb") as output:
            output.write(original.read(offset))
        tool_image = checked_plugin(plugin, base)
        tool = extract(tool_image, base / "tool") / "usr/bin/appimagetool"
        if not tool.is_file():
            raise ValueError("Pinned AppImage plugin has no appimagetool")
        repacked = base / image.name
        subprocess.run([str(tool), "--no-appstream", "--runtime-file", str(runtime),
                        str(appdir), str(repacked)], env={**os.environ, "ARCH": "x86_64"},
                       stdout=subprocess.DEVNULL, check=True, timeout=180)
        verified = extract(repacked, base / "verified")
        verify_java(verified / "usr/lib/MoleAPI")
        subprocess.run([sys.executable, str(ROOT / "scripts/check-script-worker.py"),
                        str(verified / "AppRun")], check=True, timeout=45)
        subprocess.run([sys.executable, str(ROOT / "scripts/check-bundled-dubbo.py"),
                        str(verified / "AppRun"), "--resource-root",
                        str(verified / "usr/lib/MoleAPI")], check=True, timeout=45)
        os.replace(repacked, image)
    print(f"PASS restored original JRE in {image.name}", flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle_directory", type=Path)
    parser.add_argument("--java-source", type=Path, default=ROOT / "vendor/java-runtime/current")
    parser.add_argument("--plugin", type=Path, help="Pinned plugin file already downloaded")
    args = parser.parse_args()
    repair(args.bundle_directory, args.java_source, args.plugin)
