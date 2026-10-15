#!/usr/bin/env python3
"""Run official ZooKeeper lookup then admit its actual provider URL in Rust."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import queue
import subprocess
import threading

ROOT = Path(__file__).resolve().parents[2]
JAVA_PROJECT = ROOT / 'tools/dubbo-discovery-qa/java'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--java', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--classes', type=Path, default=JAVA_PROJECT / 'target/classes')
    parser.add_argument('--dependencies', type=Path, default=JAVA_PROJECT / 'target/dependency')
    args = parser.parse_args()
    assert args.java.is_absolute() and args.java.is_file()
    assert args.jar.is_absolute() and args.jar.is_file()
    assert args.classes.is_absolute() and args.classes.is_dir()
    assert args.dependencies.is_absolute() and args.dependencies.is_dir()
    manifest = json.loads((ROOT / 'vendor/dubbo-worker/manifest.json').read_text())
    jar_bytes = args.jar.read_bytes()
    assert len(jar_bytes) == manifest['worker_bytes']
    assert hashlib.sha256(jar_bytes).hexdigest() == manifest['worker_sha256']
    build = subprocess.run(['cargo', 'build', '--locked', '-p', 'moleapi-protocols',
                            '--example', 'dubbo-discovery-fixture'], cwd=ROOT,
                           capture_output=True, text=True, timeout=600)
    assert build.returncode == 0, build.stderr[-4000:]
    example = ROOT / 'target/debug/examples/dubbo-discovery-fixture'
    classpath = os.pathsep.join((str(args.classes), str(args.dependencies / '*')))
    command = [str(args.java), '-cp', classpath, 'fixture.RegistryFixture']
    args.output.parent.mkdir(parents=True, exist_ok=True)
    held_log = args.output.with_suffix('.held.log')
    completed_log = args.output.with_suffix('.complete.log')
    with held_log.open('w') as diagnostics:
        provider = subprocess.Popen(command + ['--hold'], cwd=JAVA_PROJECT,
                                    stdout=subprocess.PIPE, stderr=diagnostics, text=True)
        results = queue.Queue(maxsize=2)
        def watch():
            for line in provider.stdout:
                if line.startswith(('ZOOKEEPER_PROVIDER=', 'ZOOKEEPER_REGISTRY=')):
                    try: results.put_nowait(line.strip())
                    except queue.Full: pass
            try: results.put_nowait(None)
            except queue.Full: pass
        threading.Thread(target=watch, daemon=True).start()
        try:
            details = dict(results.get(timeout=35).split('=', 1) for _ in range(2))
            source = details['ZOOKEEPER_PROVIDER']
            registry = details['ZOOKEEPER_REGISTRY']
            assert len(source) <= 4096 and source.startswith('dubbo://')
            result = subprocess.run([str(example), source, registry,
                                     str(args.java),str(args.jar)],cwd=ROOT,capture_output=True,
                                    text=True,timeout=40)
            assert result.returncode == 0, result.stderr[-4000:]
            assert 'PASS original ZooKeeper provider URL -> checked Rust address -> official direct RPC' in result.stdout, result.stdout
        finally:
            provider.terminate()
            try: provider.wait(timeout=12)
            except subprocess.TimeoutExpired:
                provider.kill(); provider.wait(timeout=5)
    with completed_log.open('w') as diagnostics:
        complete = subprocess.run(command,cwd=JAVA_PROJECT,stdout=subprocess.PIPE,
                                  stderr=diagnostics,text=True,timeout=60,check=True)
    assert 'PASS ZOOKEEPER lookup group version direct-call provider-removal' in complete.stdout
    evidence = {'format':'moleapi-dubbo-zookeeper-qa-v1',
                'provider_source_sha256':hashlib.sha256(source.encode()).hexdigest(),
                'worker_sha256':manifest['worker_sha256'],
                'cases':['official_lookup','group_filter','version_filter','no_lookup_invocation',
                         'checked_rust_provider_call','hosted_private_address_refusal','provider_removal']}
    args.output.write_text(json.dumps(evidence,indent=2)+'\n')
    print('PASS ZooKeeper registry -> original provider URL -> checked Rust RPC -> removal',flush=True)


if __name__ == '__main__':
    main()
