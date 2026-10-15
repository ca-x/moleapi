"""Startup watcher must read lines already buffered by TextIOWrapper."""
import importlib.util
from pathlib import Path
import subprocess
import sys
import unittest

spec = importlib.util.spec_from_file_location('verify_server', Path(__file__).with_name('verify-server.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class AnnounceTests(unittest.TestCase):
    def test_two_lines_in_one_write(self):
        process = subprocess.Popen([sys.executable, '-c',
            "import sys,time;sys.stdout.write('First-user setup token: qa\\nMoleAPI listening on http://127.0.0.1:9000\\n');sys.stdout.flush();time.sleep(1)"],
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        try:
            self.assertEqual(module.announced_address(process, timeout=2), 'http://127.0.0.1:9000')
        finally:
            process.terminate()
            process.wait(timeout=3)
            process.stdout.close()


if __name__ == '__main__':
    unittest.main()
