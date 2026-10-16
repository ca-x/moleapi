#!/usr/bin/env python3
"""Extract each Linux desktop package and smoke its actual bundled payload."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def one_package(directory: Path, suffix: str) -> Path:
    matches = list(directory.rglob(f"*{suffix}"))
    if len(matches) != 1:
        raise ValueError(f"Expected one {suffix} package, found {len(matches)}")
    return matches[0].resolve(strict=True)


def extract_rpm(package: Path, destination: Path) -> None:
    bsdtar = shutil.which("bsdtar")
    if bsdtar is None and os.environ.get("GITHUB_ACTIONS") == "true" and os.environ.get("RUNNER_OS") == "Linux":
        # The workflow already updated apt metadata before building this package.
        subprocess.run(["sudo", "apt-get", "install", "-y", "libarchive-tools"],
                       check=True, timeout=180)
        bsdtar = shutil.which("bsdtar")
    if bsdtar is None:
        raise RuntimeError("bsdtar (libarchive-tools) is required to inspect RPM payloads")
    subprocess.run([bsdtar, "-xf", str(package)], cwd=destination,
                   check=True, timeout=180)


def verify_extracted(name: str, root: Path, executable: Path) -> None:
    resources = root / "usr/lib/MoleAPI"
    if not executable.is_file() or not resources.is_dir():
        raise ValueError(f"{name}: installed executable or resources missing")
    subprocess.run([sys.executable, str(ROOT / "scripts/check-script-worker.py"), str(executable)],
                   check=True, timeout=45)
    subprocess.run([sys.executable, str(ROOT / "scripts/check-bundled-java.py"), str(executable),
                    "--resources", str(resources), "--runtime-only"], check=True, timeout=45)
    subprocess.run([sys.executable, str(ROOT / "scripts/check-bundled-dubbo.py"), str(executable),
                    "--resource-root", str(resources)], check=True, timeout=45)
    print(f"PASS extracted {name}: worker, exact JRE inventory and offline Dubbo worker", flush=True)


def main(directory: Path) -> None:
    directory = directory.resolve(strict=True)
    deb = one_package(directory, ".deb")
    rpm = one_package(directory, ".rpm")
    appimage = one_package(directory, ".AppImage")
    with tempfile.TemporaryDirectory(prefix="moleapi-linux-packages-") as temporary:
        base = Path(temporary)
        deb_root = base / "deb"
        deb_root.mkdir()
        subprocess.run(["dpkg-deb", "-x", str(deb), str(deb_root)], check=True, timeout=180)
        verify_extracted("deb", deb_root, deb_root / "usr/bin/moleapi")

        rpm_root = base / "rpm"
        rpm_root.mkdir()
        extract_rpm(rpm, rpm_root)
        verify_extracted("rpm", rpm_root, rpm_root / "usr/bin/moleapi")

        app_root = base / "appimage"
        app_root.mkdir()
        environment = {**os.environ, "APPIMAGE_EXTRACT_AND_RUN": "1"}
        subprocess.run([str(appimage), "--appimage-extract"], cwd=app_root,
                       env=environment, check=True, timeout=180,
                       stdout=subprocess.DEVNULL)
        extracted = app_root / "squashfs-root"
        verify_extracted("AppImage", extracted, extracted / "AppRun")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle_directory", type=Path)
    args = parser.parse_args()
    main(args.bundle_directory)
