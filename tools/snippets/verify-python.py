#!/usr/bin/env python3
"""Syntax-check/run original trusted Python snippets against native HTTP."""
import argparse
import base64
from email import policy
from email.parser import BytesParser
import urllib.parse
import importlib.util
import json
import os
from pathlib import Path
import platform
import queue
import subprocess
import threading

spec = importlib.util.spec_from_file_location('java_wire_qa', Path(__file__).with_name('verify-java.py'))
wire = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wire)


def verify(args):
    for path in (args.python, args.node, args.output):
        assert path.is_absolute()
    assert args.python.is_file() and args.node.is_file()
    args.output.mkdir(exist_ok=False)
    env = os.environ.copy()
    # The fixture owns its loopback endpoint; Python should not proxy it externally.
    for name in ('HTTP_PROXY', 'HTTPS_PROXY', 'http_proxy', 'https_proxy', 'ALL_PROXY', 'all_proxy'):
        env.pop(name, None)
    versions = {'python': wire.run([str(args.python), '--version'], args.output, env),
                'node': wire.run([str(args.node), '--version'], args.output, env)}
    versions['packages'] = wire.run([str(args.python), '-m', 'pip', 'freeze'], args.output, env)
    expected_packages = Path(__file__).with_name('python-requirements.txt').read_text().splitlines()
    assert sorted(versions['packages'].splitlines()) == sorted(expected_packages), 'Install the pinned Python QA requirements in an isolated venv'
    tools = {'python': wire.digest(args.python), 'node': wire.digest(args.node)}
    records = queue.Queue()
    server_file = args.output / 'server.cjs'
    server_file.write_text("""const http=require('http');const server=http.createServer((request,response)=>{const chunks=[];request.on('data',part=>chunks.push(part));request.on('end',()=>{console.log(JSON.stringify({method:request.method,path:request.url,headers:request.headers,body:Buffer.concat(chunks).toString('base64')}));response.writeHead(200,{'Content-Type':'application/json'});response.end('{"ok":true}');});});server.listen(0,'127.0.0.1',()=>console.log(JSON.stringify({port:server.address().port})));""")
    server = subprocess.Popen([str(args.node), str(server_file)], stdout=subprocess.PIPE, text=True)
    try:
        port = json.loads(server.stdout.readline())['port']
        def receive():
            for line in server.stdout:
                records.put(json.loads(line))
        receiver = threading.Thread(target=receive, daemon=True)
        receiver.start()
        evidence = []
        for profile in args.profiles.split(','):
            assert profile in ('core', 'form', 'text', 'binary', 'binary-mime', 'multipart', 'dollar', 'json-values', 'json-scalar', 'json-null', 'json-empty', 'get-binary', 'delete-json', 'text-json', 'multipart-meta', 'multipart-text')
            root = args.output / profile
            wire.run(['cargo', 'run', '--locked', '-p', 'moleapi-generation', '--example', 'fixture_snippets', '--', str(root),
                      f'http://127.0.0.1:{port}', 'python', profile], wire.ROOT, env)
            manifest = json.loads((root / 'fixture-manifest.json').read_text())
            assert manifest['target'] == 'python' and manifest['profile'] == profile
            assert wire.digest(root / 'fixture-request.json') == manifest['source_sha256']
            saved = json.loads((root / 'fixture-request.json').read_text())
            expected = json.loads(saved['data']['collections'][0]['requests'][0]['body']) if profile.startswith('json-') else None
            for client in args.clients.split(','):
                assert client in ('python3', 'aiohttp', 'requests', 'httpx_sync', 'httpx_async')
                snippet = root / (client + '.py')
                original = snippet.read_bytes()
                assert wire.digest(snippet) == manifest['files'][snippet.name]
                project = root / client
                project.mkdir()
                (project / 'upload.bin').write_bytes(wire.BYTES)
                (project / 'upload-鼹鼠.bin').write_bytes(wire.BYTES)
                wrapper = project / 'consumer.py'
                # Supply only imports/async entry point. Generated code is unchanged except indentation.
                source = original.decode()
                if client in ('httpx_async', 'aiohttp'):
                    package = 'httpx' if client == 'httpx_async' else 'aiohttp'
                    source = f'import asyncio\nimport {package}\nasync def main():\n' + '\n'.join('    ' + line for line in source.split('\n')) + '\nasyncio.run(main())\n'
                elif client != 'python3':
                    source = ('import requests\n' if client == 'requests' else 'import httpx\n') + source + '\n'
                wrapper.write_text(source)
                wire.run([str(args.python), '-m', 'py_compile', str(wrapper)], project, env)
                wire.run([str(args.python), str(wrapper)], project, env, timeout=30)
                record = records.get(timeout=10)
                (project / 'wire-evidence.json').write_text(json.dumps(record, indent=2) + '\n')
                if profile in ('multipart-meta', 'multipart-text'):
                    headers = record['headers']
                    assert record['method'] == 'POST' and headers['authorization'] == 'Bearer fixture-token'
                    assert headers['x-test'] == 'moleapi-fixture' and 'wrong.invalid' not in headers['host']
                    assert urllib.parse.parse_qs(urllib.parse.urlsplit(record['path']).query) == {'existing': ['one'], 'q': ['space & +']}
                    mime = headers['content-type']
                    assert mime.startswith('multipart/form-data;') and 'obsolete' not in mime
                    message = BytesParser(policy=policy.default).parsebytes(b'Content-Type: ' + mime.encode() + b'\r\nMIME-Version: 1.0\r\n\r\n' + base64.b64decode(record['body']))
                    parts = list(message.iter_parts())
                    expected_parts = [part for part in json.loads(saved['data']['collections'][0]['requests'][0]['body'])['parts'] if part.get('enabled', True)]
                    assert len(parts) == len(expected_parts)
                    for part, expected_part in zip(parts, expected_parts):
                        assert part.get_param('name', header='Content-Disposition') == expected_part['name']
                        value = expected_part['value']
                        if value['kind'] == 'file':
                            assert part.get_filename() == value['file']['file_name']
                            assert part.get_content_type() == value['file']['mime']
                            assert part.get_payload(decode=True) == wire.BYTES
                        else:
                            assert part.get_content_type() == (value['mime'] or 'text/plain')
                            assert part.get_payload(decode=True) == value['text'].encode()
                elif profile.startswith('json-'):
                    headers = record['headers']
                    assert record['method'] == 'POST' and headers['authorization'] == 'Bearer fixture-token'
                    assert headers['x-test'] == 'moleapi-fixture' and 'wrong.invalid' not in headers['host']
                    assert urllib.parse.parse_qs(urllib.parse.urlsplit(record['path']).query) == {'existing': ['one'], 'q': ['space & +']}
                    assert headers['content-type'].startswith('application/json')
                    observed = json.loads(base64.b64decode(record['body']))
                    assert json.dumps(observed, sort_keys=True) == json.dumps(expected, sort_keys=True)
                else:
                    wire.check('binary' if profile == 'get-binary' else 'core' if profile == 'delete-json' else 'text' if profile in ('dollar', 'text-json') else profile, record,
                               expected_text='literal $ and ${1 + 2} 鼹鼠' if profile == 'dollar' else saved['data']['collections'][0]['requests'][0]['body'] if profile == 'text-json' else None,
                               expected_method='GET' if profile == 'get-binary' else 'DELETE' if profile == 'delete-json' else 'POST')
                assert records.empty() and snippet.read_bytes() == original
                item = {'client': client, 'profile': profile, 'platform': platform.platform(), 'versions': versions,
                        'tool_sha256': tools, 'engine_sha256': wire.digest(wire.ROOT / 'vendor/snippet-engine/engine.js'),
                        'source_sha256': manifest['source_sha256'], 'snippet_sha256': manifest['files'][snippet.name], 'result': 'passed'}
                (project / 'runtime-evidence.json').write_text(json.dumps(item, indent=2) + '\n')
                evidence.append(item)
                print(f'PASS Python/{client}/{profile}: original syntax/native request and exact body', flush=True)
        (args.output / 'runtime-evidence.json').write_text(json.dumps(evidence, indent=2) + '\n')
    finally:
        server.terminate()
        server.wait(timeout=10)
        server.stdout.close()
        if 'receiver' in locals():
            receiver.join(timeout=2)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--python', type=Path, required=True)
    parser.add_argument('--node', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--profiles', default='core,form,text,binary,binary-mime,multipart,dollar,json-values,json-scalar,json-null,json-empty,get-binary,delete-json,text-json,multipart-meta,multipart-text')
    parser.add_argument('--clients', default='python3,aiohttp,requests,httpx_sync,httpx_async')
    verify(parser.parse_args())
