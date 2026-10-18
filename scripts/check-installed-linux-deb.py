#!/usr/bin/env python3
"""Install the built Linux deb on an ephemeral CI runner and check installed resources."""
import argparse
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
INSTALLED_BINARY = Path('/usr/bin/moleapi')
INSTALLED_RESOURCES = Path('/usr/lib/MoleAPI')


def main(bundle_directory: Path, environment=None) -> None:
    environment = os.environ if environment is None else environment
    if environment.get('GITHUB_ACTIONS') != 'true' or environment.get('RUNNER_OS') != 'Linux':
        raise RuntimeError('Installed deb smoke requires a GitHub Actions Linux runner')
    bundle_directory = bundle_directory.resolve(strict=True)
    packages = list(bundle_directory.rglob('*.deb'))
    if len(packages) != 1:
        raise ValueError(f'Expected exactly one built deb package, found {len(packages)}')
    package = packages[0].resolve(strict=True)
    metadata = subprocess.run(['dpkg-deb', '--field', str(package), 'Package'],
                              capture_output=True, text=True, check=True, timeout=15)
    if metadata.stdout.strip() != 'moleapi':
        raise ValueError('Expected the MoleAPI deb package before installation')
    subprocess.run(['sudo', 'apt-get', 'install', '-y', '--no-install-recommends', str(package)],
                   check=True, timeout=300)
    status = subprocess.run(['dpkg-query', '-W', '-f=${Status}', 'moleapi'],
                            capture_output=True, text=True, check=True, timeout=15)
    if status.stdout.strip() != 'install ok installed':
        raise RuntimeError('MoleAPI deb is not installed')
    if not INSTALLED_BINARY.is_file() or not INSTALLED_RESOURCES.is_dir():
        raise RuntimeError('Installed MoleAPI executable or resources missing')
    clean = {key: value for key, value in os.environ.items() if key not in
             {'LD_LIBRARY_PATH', 'JAVA_HOME', 'MOLEAPI_CODEGEN_JAVA', 'MOLEAPI_DUBBO_WORKER_JAR'}}
    checks = [
        [sys.executable, str(ROOT / 'scripts/check-script-worker.py'), str(INSTALLED_BINARY)],
        [sys.executable, str(ROOT / 'scripts/check-bundled-java.py'), str(INSTALLED_BINARY),
         '--resources', str(INSTALLED_RESOURCES), '--runtime-only'],
        [sys.executable, str(ROOT / 'scripts/check-bundled-dubbo.py'), str(INSTALLED_BINARY),
         '--resource-root', str(INSTALLED_RESOURCES)],
    ]
    for command in checks:
        subprocess.run(command, env=clean, check=True, timeout=60)
    print('PASS installed Linux deb: package manager, executable, script worker, exact Java and Dubbo resources', flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('bundle_directory', type=Path)
    args = parser.parse_args()
    main(args.bundle_directory)
