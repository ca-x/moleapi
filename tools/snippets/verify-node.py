#!/usr/bin/env python3
"""Run original trusted modern Node snippets against native HTTP, never user code."""
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
    for path in (args.node, args.dependencies, args.output):
        assert path.is_absolute()
    assert args.node.is_file()
    args.output.mkdir(exist_ok=False)
    env = os.environ.copy()
    # The fixture owns its loopback endpoint; Node should not proxy it externally.
    for name in ('HTTP_PROXY', 'HTTPS_PROXY', 'http_proxy', 'https_proxy', 'ALL_PROXY', 'all_proxy'):
        env.pop(name, None)
    assert (args.dependencies / 'node_modules').is_dir()
    lock = Path(__file__).with_name('node-runtime') / 'package-lock.json'
    assert (args.dependencies / 'package-lock.json').read_bytes() == lock.read_bytes(), 'Install the pinned Node QA lock in an isolated directory'
    versions = {'node': wire.run([str(args.node), '--version'], args.output, env), 'libraries': {}}
    declared = json.loads(lock.with_name('package.json').read_text())['dependencies']
    for name, version in declared.items():
        package = json.loads((args.dependencies / 'node_modules' / name / 'package.json').read_text())
        assert package['version'] == version
        versions['libraries'][name] = version
    tools = {'node': wire.digest(args.node), 'dependency_lock': wire.digest(lock)}
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
            assert profile in ('core', 'form', 'text', 'binary', 'binary-mime', 'multipart', 'dollar', 'json-values', 'json-scalar', 'json-null', 'json-empty', 'get-binary', 'head-binary', 'delete-json', 'text-json', 'multipart-meta', 'multipart-text', 'json-object', 'headers-quoted')
            root = args.output / profile
            wire.run(['cargo', 'run', '--locked', '-p', 'moleapi-generation', '--example', 'fixture_snippets', '--', str(root),
                      f'http://127.0.0.1:{port}', 'node', profile], wire.ROOT, env)
            manifest = json.loads((root / 'fixture-manifest.json').read_text())
            assert manifest['target'] == 'node' and manifest['profile'] == profile
            assert wire.digest(root / 'fixture-request.json') == manifest['source_sha256']
            saved = json.loads((root / 'fixture-request.json').read_text())
            expected = json.loads(saved['data']['collections'][0]['requests'][0]['body']) if profile.startswith('json-') else None
            for client in args.clients.split(','):
                assert client in ('fetch', 'axios', 'ofetch', 'undici')
                project = root / client
                project.mkdir()
                (project / 'node_modules').symlink_to(args.dependencies / 'node_modules', target_is_directory=True)
                (project / 'upload.bin').write_bytes(wire.BYTES)
                (project / "upload-鼹鼠'quote.bin").write_bytes(wire.BYTES)
                if client in manifest['rejections']:
                    assert client in ('fetch', 'ofetch') and profile in ('get-binary', 'head-binary')
                    assert 'cannot send GET/HEAD bodies' in manifest['rejections'][client]
                    assert (client + '.mjs') not in manifest['files']
                    method = 'GET' if profile == 'get-binary' else 'HEAD'
                    probe = project / 'native-rejection.mjs'
                    prelude = "import { ofetch } from 'ofetch'\n" if client == 'ofetch' else ''
                    # A trusted direct native API probe confirms the application rejection is real.
                    probe.write_text(prelude + 'let rejected = false;\ntry { await '+('ofetch' if client == 'ofetch' else 'fetch')+'('+json.dumps(f'http://127.0.0.1:{port}/echo')+', { method: '+json.dumps(method)+', body: Buffer.from([0,255,128,10,88]) }); } catch(error) { rejected = true; console.log(JSON.stringify({error:error.message})); }\nif (!rejected) throw new Error("Native API unexpectedly accepted body");\n')
                    wire.run([str(args.node), '--check', str(probe)], project, env)
                    native_error = json.loads(wire.run([str(args.node), str(probe)], project, env, timeout=30))['error']
                    assert 'cannot have body' in native_error.lower() and records.empty()
                    item = {'client':client, 'profile':profile, 'result':'rejected', 'application_error':manifest['rejections'][client], 'native_error':native_error, 'source_sha256':manifest['source_sha256'], 'engine_sha256':wire.digest(wire.ROOT / 'vendor/snippet-engine/engine.js'), 'versions':versions, 'tool_sha256':tools}
                    (project / 'runtime-evidence.json').write_text(json.dumps(item,indent=2)+'\n')
                    evidence.append(item)
                    print(f'PASS Node/{client}/{profile}: generation and native API reject GET/HEAD body',flush=True)
                    continue
                snippet = root / (client + '.mjs')
                original = snippet.read_bytes()
                assert wire.digest(snippet) == manifest['files'][snippet.name]
                wrapper = project / 'consumer.mjs'
                assertion = "\nimport assert from 'node:assert/strict'\n"
                if client == 'fetch': assertion += 'assert.equal(response.status,200); assert.deepEqual(await response.json(),{ok:true});\n'
                elif client == 'ofetch': assertion += 'assert.deepEqual(response,{ok:true});\n'
                elif client == 'undici': assertion += 'assert.equal(statusCode,200); assert.equal(await body.text(), '+json.dumps('' if profile == 'head-binary' else '{"ok":true}')+');\n'
                # Native source stays byte-identical; only response assertions follow.
                wrapper.write_text(original.decode() + assertion)
                wire.run([str(args.node), '--check', str(wrapper)], project, env)
                print('RUN',args.node,wrapper,flush=True)
                result = subprocess.run([str(args.node), str(wrapper)],cwd=project,env=env,capture_output=True,text=True,timeout=30)
                assert result.returncode == 0 and not result.stderr, result.stdout+'\n'+result.stderr
                if client == 'axios' and profile != 'head-binary': assert '{ ok: true }' in result.stdout,result.stdout
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
                            assert part.get_filename() is None, 'A canonical text field must not turn into a file part'
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
                    wire.check('binary' if profile in ('get-binary', 'head-binary') else 'core' if profile == 'delete-json' else 'core' if profile == 'headers-quoted' else 'text' if profile in ('dollar', 'text-json') else profile, record,
                               expected_text='literal $ and ${1 + 2} 鼹鼠' if profile == 'dollar' else saved['data']['collections'][0]['requests'][0]['body'] if profile == 'text-json' else None,
                               expected_method='HEAD' if profile == 'head-binary' else 'GET' if profile == 'get-binary' else 'DELETE' if profile == 'delete-json' else 'POST')
                if profile == 'headers-quoted':
                    assert record['headers']["x-fixture'key"] == "literal ' \\ $fixture"
                assert records.empty() and snippet.read_bytes() == original
                item = {'client': client, 'profile': profile, 'platform': platform.platform(), 'versions': versions,
                        'tool_sha256': tools, 'engine_sha256': wire.digest(wire.ROOT / 'vendor/snippet-engine/engine.js'),
                        'source_sha256': manifest['source_sha256'], 'snippet_sha256': manifest['files'][snippet.name], 'result': 'passed'}
                (project / 'runtime-evidence.json').write_text(json.dumps(item, indent=2) + '\n')
                evidence.append(item)
                print(f'PASS Node/{client}/{profile}: original syntax/native request and exact body', flush=True)
        (args.output / 'runtime-evidence.json').write_text(json.dumps(evidence, indent=2) + '\n')
    finally:
        server.terminate()
        server.wait(timeout=10)
        server.stdout.close()
        if 'receiver' in locals():
            receiver.join(timeout=2)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--node', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--profiles', default='core,form,text,binary,binary-mime,multipart,dollar,json-values,json-scalar,json-null,json-empty,get-binary,head-binary,delete-json,text-json,multipart-meta,multipart-text,json-object,headers-quoted')
    parser.add_argument('--clients', default='fetch,axios,ofetch,undici')
    parser.add_argument('--dependencies', type=Path, required=True)
    verify(parser.parse_args())
