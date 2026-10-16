#!/usr/bin/env python3
"""Compare Apache provider metadata with a Rust checked ZooKeeper snapshot."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import queue
import subprocess
import threading

ROOT = Path(__file__).resolve().parents[2]
JAVA_PROJECT = ROOT / 'tools/dubbo-metadata-qa/java'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--java', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--classes', type=Path, default=JAVA_PROJECT / 'target/classes')
    parser.add_argument('--dependencies', type=Path, default=JAVA_PROJECT / 'target/dependency')
    args = parser.parse_args()
    assert args.java.is_absolute() and args.java.is_file()
    assert args.classes.is_absolute() and args.classes.is_dir()
    assert args.dependencies.is_absolute() and args.dependencies.is_dir()
    build = subprocess.run(['cargo','build','--locked','-p','moleapi-protocols',
                            '--example','dubbo-metadata-fixture'], cwd=ROOT,
                           capture_output=True,text=True,timeout=600)
    assert build.returncode == 0, build.stderr[-4000:]
    example = ROOT / 'target/debug/examples/dubbo-metadata-fixture'
    classpath = os.pathsep.join((str(args.classes), str(args.dependencies / '*')))
    command = [str(args.java), '-cp', classpath, 'fixture.MetadataFixture']
    args.output.parent.mkdir(parents=True, exist_ok=True)
    def held(mode, label):
        with args.output.with_suffix(f'.{label}.log').open('w') as diagnostics:
            provider = subprocess.Popen(command + [mode], cwd=JAVA_PROJECT,
                stdout=subprocess.PIPE, stderr=diagnostics, text=True)
            results = queue.Queue(maxsize=3)
            def watch():
                for line in provider.stdout:
                    if line.startswith(('METADATA_REGISTRY=', 'METADATA_PATH=', 'METADATA_JSON=')):
                        try: results.put_nowait(line.strip())
                        except queue.Full: pass
                try: results.put_nowait(None)
                except queue.Full: pass
            threading.Thread(target=watch, daemon=True).start()
            try:
                details = dict(results.get(timeout=40).split('=',1) for _ in range(3))
                endpoint = details['METADATA_REGISTRY']
                path = details['METADATA_PATH']
                original = details['METADATA_JSON']
                assert endpoint.startswith('zookeeper://127.0.0.1:')
                assert path == '/dubbo/metadata/fixture.RegistryService/1.0.0/qa/provider/moleapi-metadata-qa'
                assert len(original) <= 1024*1024
                invoke = [str(example),endpoint,original]
                if mode == '--auth-hold': invoke.append('--auth')
                reply = subprocess.run(invoke, cwd=ROOT,
                    capture_output=True,text=True,timeout=40)
                assert reply.returncode == 0, reply.stderr[-4000:]
                assert 'PASS official Apache metadata document -> checked Rust preview' in reply.stdout
                return original
            finally:
                provider.terminate()
                try: provider.wait(timeout=12)
                except subprocess.TimeoutExpired:
                    provider.kill(); provider.wait(timeout=5)
    original = held('--hold','held')
    secured = held('--auth-hold','auth')
    for mode,label in [(None,'complete'),('--auth','auth-complete')]:
        with args.output.with_suffix(f'.{label}.log').open('w') as diagnostics:
            complete = subprocess.run(command + ([mode] if mode else []), cwd=JAVA_PROJECT,
                stdout=subprocess.PIPE, stderr=diagnostics,text=True,timeout=60)
        assert complete.returncode == 0, complete.stdout[-2000:]
        assert 'PASS APACHE metadata report provider-definition no-invocation' in complete.stdout
    evidence = {'format':'moleapi-dubbo-metadata-qa-v1',
                'definition_sha256':hashlib.sha256(original.encode()).hexdigest(),
                'secured_definition_sha256':hashlib.sha256(secured.encode()).hexdigest(),
                'cases':['official_metadata_report','method_parameter_response_types',
                         'dto_property_mapping',
                         'checked_rust_metadata_read','hosted_private_refusal',
                         'no_lookup_invocation','digest_auth','anonymous_auth_refusal',
                         'wrong_digest_refusal']}
    args.output.write_text(json.dumps(evidence,indent=2)+'\n')
    print('PASS Apache provider metadata -> checked Rust preview',flush=True)


if __name__ == '__main__': main()
