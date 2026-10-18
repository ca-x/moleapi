#!/usr/bin/env python3
"""Safe fixtures for the CI-only installed deb gate."""
import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

source = Path(__file__).with_name('check-installed-linux-deb.py')
spec = importlib.util.spec_from_file_location('installed_deb', source)
installed = importlib.util.module_from_spec(spec)
spec.loader.exec_module(installed)


class InstalledDebSmoke(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.bundle = self.root / 'bundle'
        self.bundle.mkdir()
        self.binary = self.root / 'usr/bin/moleapi'
        self.resources = self.root / 'usr/lib/MoleAPI'
        self.binary.parent.mkdir(parents=True)
        self.binary.write_bytes(b'fixture')
        self.resources.mkdir(parents=True)
        self.ci = {'GITHUB_ACTIONS': 'true', 'RUNNER_OS': 'Linux'}

    def test_local_invocation_refuses_before_package_manager_calls(self):
        (self.bundle / 'moleapi.deb').write_bytes(b'fixture')
        with patch.object(installed.subprocess, 'run') as run:
            with self.assertRaisesRegex(RuntimeError, 'GitHub Actions Linux'):
                installed.main(self.bundle, {'GITHUB_ACTIONS': 'false', 'RUNNER_OS': 'Linux'})
            run.assert_not_called()

    def test_missing_or_ambiguous_packages_refuse_before_install(self):
        with patch.object(installed.subprocess, 'run') as run:
            with self.assertRaisesRegex(ValueError, 'exactly one'):
                installed.main(self.bundle, self.ci)
            (self.bundle / 'one.deb').write_bytes(b'fixture')
            (self.bundle / 'two.deb').write_bytes(b'fixture')
            with self.assertRaisesRegex(ValueError, 'exactly one'):
                installed.main(self.bundle, self.ci)
            run.assert_not_called()

    def test_installs_exact_deb_and_checks_installed_paths(self):
        package = self.bundle / 'moleapi.deb'
        package.write_bytes(b'fixture')
        outputs = [
            subprocess.CompletedProcess([], 0, stdout='moleapi\n'),
            subprocess.CompletedProcess([], 0),
            subprocess.CompletedProcess([], 0, stdout='install ok installed'),
            *[subprocess.CompletedProcess([], 0) for _ in range(3)],
        ]
        with patch.object(installed, 'INSTALLED_BINARY', self.binary), patch.object(installed, 'INSTALLED_RESOURCES', self.resources), patch.object(installed.subprocess, 'run', side_effect=outputs) as run:
            installed.main(self.bundle, self.ci)
        commands = [call.args[0] for call in run.call_args_list]
        self.assertEqual(commands[0], ['dpkg-deb', '--field', str(package), 'Package'])
        self.assertEqual(commands[1], ['sudo', 'apt-get', 'install', '-y', '--no-install-recommends', str(package)])
        self.assertEqual(commands[2], ['dpkg-query', '-W', '-f=${Status}', 'moleapi'])
        self.assertEqual(commands[3:], [
            [sys.executable, str(installed.ROOT / 'scripts/check-script-worker.py'), str(self.binary)],
            [sys.executable, str(installed.ROOT / 'scripts/check-bundled-java.py'), str(self.binary),
             '--resources', str(self.resources), '--runtime-only'],
            [sys.executable, str(installed.ROOT / 'scripts/check-bundled-dubbo.py'), str(self.binary),
             '--resource-root', str(self.resources)],
        ])
        self.assertTrue(all(call.kwargs['check'] for call in run.call_args_list))

    def test_missing_installed_resources_refuses_smoke(self):
        (self.bundle / 'moleapi.deb').write_bytes(b'fixture')
        outputs = [
            subprocess.CompletedProcess([], 0, stdout='moleapi\n'),
            subprocess.CompletedProcess([], 0),
            subprocess.CompletedProcess([], 0, stdout='install ok installed'),
        ]
        missing = self.root / 'missing-resources'
        with patch.object(installed, 'INSTALLED_BINARY', self.binary), patch.object(installed, 'INSTALLED_RESOURCES', missing), patch.object(installed.subprocess, 'run', side_effect=outputs) as run:
            with self.assertRaisesRegex(RuntimeError, 'resources missing'):
                installed.main(self.bundle, self.ci)
        self.assertEqual(run.call_count, 3)

    def test_unexpected_package_name_refuses_install(self):
        (self.bundle / 'other.deb').write_bytes(b'fixture')
        with patch.object(installed.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, stdout='unrelated\n')) as run:
            with self.assertRaisesRegex(ValueError, 'MoleAPI'):
                installed.main(self.bundle, self.ci)
            self.assertEqual(run.call_count, 1)


if __name__ == '__main__':
    unittest.main()
