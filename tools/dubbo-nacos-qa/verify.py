#!/usr/bin/env python3
"""Verify a pinned Nacos 2.5.1 server, official Apache registry, and checked Rust route."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import queue
import subprocess
import tarfile
import tempfile
import threading
import time
import urllib.request
import urllib.parse

ROOT = Path(__file__).resolve().parents[2]
JAVA_PROJECT = ROOT / 'tools/dubbo-nacos-qa/java'
SERVER_SHA256 = 'e0877b8e5e8f234694f595a0f25b978c3745325e8b17625b7ab95661e5d3d38e'
SERVER_BYTES = 153752952
HEALTH = 'http://127.0.0.1:18848/nacos/v1/console/health/readiness'


def ready():
    with urllib.request.urlopen(HEALTH, timeout=2) as reply:
        return reply.status == 200 and reply.read(32) == b'OK'


def registered_instances():
    query = urllib.parse.urlencode({'serviceName':'providers:fixture.RegistryService:1.0.0:qa',
                                     'groupName':'DEFAULT_GROUP','healthyOnly':'false'})
    with urllib.request.urlopen(f'http://127.0.0.1:18848/nacos/v1/ns/instance/list?{query}',timeout=3) as reply:
        return json.load(reply)['hosts']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--java', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--archive', type=Path)
    parser.add_argument('--classes', type=Path, default=JAVA_PROJECT / 'target/classes')
    parser.add_argument('--dependencies', type=Path, default=JAVA_PROJECT / 'target/dependency')
    args = parser.parse_args()
    assert args.java.is_absolute() and args.java.is_file()
    assert args.jar.is_absolute() and args.jar.is_file()
    assert args.classes.is_absolute() and args.classes.is_dir()
    assert args.dependencies.is_absolute() and args.dependencies.is_dir()
    manifest = json.loads((ROOT / 'vendor/dubbo-worker/manifest.json').read_text())
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == manifest['worker_sha256']
    build = subprocess.run(['cargo','build','--locked','-p','moleapi-protocols',
                            '--example','dubbo-nacos-fixture'], cwd=ROOT,
                           capture_output=True,text=True,timeout=600)
    assert build.returncode == 0, build.stderr[-4000:]
    example = ROOT / 'target/debug/examples/dubbo-nacos-fixture'
    classpath = os.pathsep.join((str(args.classes), str(args.dependencies / '*')))
    command = [str(args.java), '-cp', classpath, 'fixture.NacosFixture']
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='moleapi-nacos-qa-') as temp:
        server = None
        if args.archive:
            assert args.archive.is_file() and args.archive.stat().st_size == SERVER_BYTES
            assert hashlib.sha256(args.archive.read_bytes()).hexdigest() == SERVER_SHA256
            with tarfile.open(args.archive, 'r:gz') as source:
                source.extractall(temp, filter='data')
            home = Path(temp) / 'nacos'
            server_log = args.output.with_suffix('.server.log').open('w')
            server = subprocess.Popen([str(args.java), '-Xms256m', '-Xmx512m', '-Xmn128m',
                '-Dnacos.standalone=true', f'-Dnacos.home={home}',
                f'-Dloader.path={home}/plugins', '-jar', str(home / 'target/nacos-server.jar'),
                f'--spring.config.additional-location=file:{home}/conf/', '--server.port=18848',
                f'--logging.config={home}/conf/nacos-logback.xml'],
                cwd=home, stdout=server_log, stderr=subprocess.STDOUT)
        try:
            for _ in range(90):
                try:
                    if ready(): break
                except Exception:
                    pass
                if server and server.poll() is not None:
                    raise RuntimeError('Pinned Nacos server exited before readiness')
                time.sleep(.5)
            else: raise RuntimeError('Nacos server did not become ready')
            with args.output.with_suffix('.held.log').open('w') as diagnostics:
                provider = subprocess.Popen(command + ['--hold'], cwd=JAVA_PROJECT,
                    stdout=subprocess.PIPE, stderr=diagnostics, text=True)
                results = queue.Queue(maxsize=2)
                def watch():
                    for line in provider.stdout:
                        if line.startswith(('NACOS_PROVIDER=', 'NACOS_REGISTRY=')):
                            try: results.put_nowait(line.strip())
                            except queue.Full: pass
                    try: results.put_nowait(None)
                    except queue.Full: pass
                threading.Thread(target=watch, daemon=True).start()
                try:
                    details = dict(results.get(timeout=45).split('=',1) for _ in range(2))
                    original = details['NACOS_PROVIDER']
                    registry = details['NACOS_REGISTRY']
                    assert registry == 'nacos://127.0.0.1:18848'
                    result = subprocess.run([str(example), original, registry,
                        str(args.java), str(args.jar)], cwd=ROOT,
                        capture_output=True,text=True,timeout=40)
                    assert result.returncode == 0, result.stderr[-4000:]
                    assert 'PASS official Nacos provider URL -> checked HTTP snapshot -> product Dubbo RPC' in result.stdout
                finally:
                    provider.terminate()
                    try: provider.wait(timeout=15)
                    except subprocess.TimeoutExpired:
                        provider.kill(); provider.wait(timeout=5)
            for _ in range(80):
                if not registered_instances(): break
                time.sleep(.5)
            else: raise RuntimeError('Held Nacos provider remained registered after exit')
            with args.output.with_suffix('.complete.log').open('w') as diagnostics:
                complete = subprocess.run(command, cwd=JAVA_PROJECT, stdout=subprocess.PIPE,
                    stderr=diagnostics,text=True,timeout=80)
            assert complete.returncode == 0, complete.stdout[-2000:]
            assert 'PASS NACOS lookup group version provider-removal' in complete.stdout
        finally:
            if server:
                server.terminate()
                try: server.wait(timeout=12)
                except subprocess.TimeoutExpired:
                    server.kill(); server.wait(timeout=5)
                server_log.close()
    evidence = {'format':'moleapi-dubbo-nacos-qa-v1',
                'server_sha256':SERVER_SHA256 if args.archive else 'external',
                'provider_source_sha256':hashlib.sha256(original.encode()).hexdigest(),
                'worker_sha256':manifest['worker_sha256'],
                'cases':['official_lookup','group_filter','version_filter','registered_instance_metadata',
                         'exact_provider_url_mapping','checked_http_snapshot','product_registry_rpc',
                         'hosted_registry_refusal','provider_removal']}
    args.output.write_text(json.dumps(evidence,indent=2)+'\n')
    print('PASS Nacos HTTP snapshot -> official provider URL -> checked Dubbo RPC -> removal',flush=True)


if __name__ == '__main__': main()
