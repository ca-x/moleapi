#!/usr/bin/env python3
"""Call the original bounded Java worker against the official QA provider."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--java', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--port', type=int, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    assert args.java.is_absolute() and args.java.is_file()
    assert args.jar.is_absolute() and args.jar.is_file()
    assert 1 <= args.port <= 65535
    base = dict(format='moleapi-dubbo-call-v1',
                endpoint=f'dubbo://127.0.0.1:{args.port}/fixture.EchoService',
                service='fixture.EchoService', version='1.0.0', group='', timeout_ms=5000,
                attachments={})
    cases = [
        ('echo', ['java.lang.String'], ['"moleapi"'], 'echo:moleapi'),
        ('join', ['java.lang.String', 'java.lang.String'], ['"first"', '"second"'], 'first|second'),
        ('add', ['long', 'long'], ['"9007199254740993"', '2'], 9007199254740995),
        ('describe', ['fixture.Greeting'], ['{"$class":"fixture.Greeting","$":{"name":"鼹鼠","count":0}}'], '鼹鼠:0'),
        ('names', ['java.util.List'], ['["first","鼹鼠"]'], 'first|鼹鼠'),
        ('maybeNull', ['java.lang.String'], ['"null"'], None),
    ]
    results = []

    def call(source):
        raw = json.dumps(source, ensure_ascii=False, separators=(',', ':')).encode()
        completed = subprocess.run([str(args.java), '-Xmx128m', '-Djava.awt.headless=true',
                                    '-jar', str(args.jar)], input=raw, capture_output=True,
                                   timeout=25, check=True)
        assert len(completed.stdout) <= 1024 * 1024, 'worker output exceeded limit'
        return json.loads(completed.stdout)

    for method, types, sources, expected in cases:
        response = call({**base, 'method': method, 'parameter_types': types, 'argument_sources': sources})
        assert response == {'ok': True, 'value': expected}, (method, response)
        results.append({'case': method, 'passed': True})
        print('PASS', method, flush=True)
    failed = call({**base, 'method':'fail', 'parameter_types':['java.lang.String'],
                   'argument_sources':['"expected"']})
    assert failed['ok'] is False and failed['kind'] == 'remote_exception' and 'fixture:expected' in failed['message'], failed
    results.append({'case':'remote_exception', 'passed':True})
    rejected = call({**base, 'endpoint':'dubbo://example.com:20880/fixture.EchoService',
                     'method':'echo', 'parameter_types':['java.lang.String'],
                     'argument_sources':['"moleapi"']})
    assert rejected['ok'] is False and rejected['kind'] == 'worker_error' and 'literal IP' in rejected['message'], rejected
    results.append({'case':'reject_unchecked_hostname', 'passed':True})
    ipv6 = call({**base, 'endpoint':'dubbo://[::1]:1/fixture.EchoService',
                 'timeout_ms':500, 'method':'echo', 'parameter_types':['java.lang.String'],
                 'argument_sources':['"moleapi"']})
    assert ipv6['ok'] is False and 'literal IP' not in ipv6['message'], ipv6
    results.append({'case':'accept_ipv6_literal', 'passed':True})
    evidence = {'format':'moleapi-dubbo-worker-qa-v1',
                'jar_sha256':hashlib.sha256(args.jar.read_bytes()).hexdigest(),
                'cases':results}
    args.output.write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + '\n')
    print('PASS bounded official Dubbo generic worker', flush=True)


if __name__ == '__main__':
    main()
