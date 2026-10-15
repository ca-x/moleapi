#!/usr/bin/env python3
"""Verify staged classic Dubbo JAR/JRE bytes and a real offline worker start."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
WORKER = json.loads((ROOT / 'vendor/dubbo-worker/manifest.json').read_text())
JAVA = json.loads((ROOT / 'vendor/java-runtime/manifest.json').read_text())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path, help='Staged application executable')
    parser.add_argument('--resource-root', type=Path, help='Tauri resource root; default is executable directory')
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    resources = (args.resource_root or binary.parent).resolve(strict=True)
    jar = resources / 'runtime/dubbo/engine.jar'
    receipt = json.loads((resources / 'runtime/dubbo/moleapi-dubbo-worker.json').read_text())
    if not jar.is_file() or jar.stat().st_size != WORKER['worker_bytes']:
        raise SystemExit('Staged Dubbo worker is absent or has wrong size')
    if hashlib.sha256(jar.read_bytes()).hexdigest() != WORKER['worker_sha256']:
        raise SystemExit('Staged Dubbo worker hash mismatch')
    for key in ('format','dubbo_version','worker_sha256','worker_bytes'):
        if receipt.get(key) != WORKER[key]: raise SystemExit(f'Dubbo receipt mismatch: {key}')
    java_receipt = json.loads((resources / 'runtime/java/moleapi-java-runtime.json').read_text())
    if java_receipt.get('format') != 'moleapi-java-runtime-v1':
        raise SystemExit('Staged Java receipt missing')
    target = java_receipt['target']
    asset = JAVA['assets'][target]
    if java_receipt['version'] != JAVA['version'] or java_receipt['archive_sha256'] != asset['sha256']:
        raise SystemExit('Staged Java release mismatch')
    java = resources / 'runtime/java' / asset['java']
    if not java.is_file(): raise SystemExit('Staged Java executable missing')
    request = {'format':'moleapi-dubbo-call-v1','endpoint':'dubbo://example.com:20880/fixture.EchoService',
               'service':'fixture.EchoService','method':'echo','version':'1.0.0','group':'',
               'timeout_ms':1000,'parameter_types':['java.lang.String'],
               'argument_sources':['"offline"'],'attachments':{}}
    result = subprocess.run([str(java), '-Xmx128m', '-jar', str(jar)],
                            input=json.dumps(request).encode(), capture_output=True, timeout=20, check=True)
    if len(result.stdout) > 1024 * 1024: raise SystemExit('Dubbo worker response exceeded limit')
    output = json.loads(result.stdout)
    if output.get('ok') is not False or output.get('kind') != 'worker_error' or 'literal IP' not in output.get('message',''):
        raise SystemExit('Staged Dubbo worker did not refuse unchecked hostnames')
    print(f'PASS bundled Dubbo {WORKER["dubbo_version"]}: {target}, {WORKER["worker_sha256"]}')


if __name__ == '__main__':
    main()
