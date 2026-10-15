#!/usr/bin/env python3
"""Run original trusted Rust Reqwest snippets against native HTTP, never user code."""
import argparse
import shutil
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
    for path in (args.node, args.output):
        assert path.is_absolute()
    assert args.node.is_file()
    args.output.mkdir(exist_ok=False)
    env = os.environ.copy()
    # The fixture owns its loopback endpoint; Node should not proxy it externally.
    for name in ('HTTP_PROXY', 'HTTPS_PROXY', 'http_proxy', 'https_proxy', 'ALL_PROXY', 'all_proxy'):
        env.pop(name, None)
    build_env = os.environ.copy()
    versions = {'node': wire.run([str(args.node), '--version'], args.output, env),
                'cargo': wire.run(['cargo', '--version'], args.output, build_env),
                'rustc': wire.run(['rustc', '--version', '--verbose'], args.output, build_env)}
    qa = Path(__file__).with_name('rust-runtime')
    sysroot = Path(wire.run(['rustc','--print','sysroot'],args.output,build_env).strip())
    tools = {'node': wire.digest(args.node), 'cargo_lock': wire.digest(qa / 'Cargo.lock'), 'rustc':wire.digest(sysroot / 'bin/rustc'), 'cargo':wire.digest(sysroot / 'bin/cargo')}
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
        fixtures = []
        for profile in args.profiles.split(','):
            assert profile in ('core', 'form', 'text', 'binary', 'binary-mime', 'multipart', 'dollar', 'json-values', 'json-scalar', 'json-null', 'json-empty', 'get-binary', 'head-binary', 'delete-json', 'text-json', 'multipart-meta', 'multipart-text', 'json-object', 'headers-quoted', 'json-controls', 'form-empty')
            root = args.output / profile
            wire.run(['cargo', 'run', '--locked', '-p', 'moleapi-generation', '--example', 'fixture_snippets', '--', str(root),
                      f'http://127.0.0.1:{port}', 'rust', profile], wire.ROOT, build_env)
            manifest = json.loads((root / 'fixture-manifest.json').read_text())
            assert manifest['target'] == 'rust' and manifest['profile'] == profile
            assert wire.digest(root / 'fixture-request.json') == manifest['source_sha256']
            saved = json.loads((root / 'fixture-request.json').read_text())
            expected = json.loads(saved['data']['collections'][0]['requests'][0]['body']) if profile.startswith('json-') else None
            snippet = root / 'snippet.rs'
            original = snippet.read_bytes()
            assert wire.digest(snippet) == manifest['files']['snippet.rs']
            fixtures.append((profile,root,manifest,saved,expected,snippet,original))
        project = args.output / 'consumer'
        (project / 'src').mkdir(parents=True)
        for name in ('Cargo.toml', 'Cargo.lock'): shutil.copy2(qa / name, project / name)
        functions = []
        for index, (profile,root,manifest,saved,expected,snippet,original) in enumerate(fixtures):
            # Original emitted statements are unchanged; only async function/response assertions surround them.
            functions.append(f'async fn fixture_{index}(expected: &str) -> Result<(), Box<dyn std::error::Error>> {{\n' + original.decode() + '\nassert_eq!(response.status().as_u16(), 200);\nassert_eq!(response.text().await?, expected);\nOk(())\n}\n')
        source = '\n'.join(functions)+'\n#[tokio::main]\nasync fn main() -> Result<(),Box<dyn std::error::Error>> {\nlet index:usize=std::env::args().nth(1).unwrap().parse()?;\nlet expected=std::env::args().nth(2).unwrap();\nmatch index {\n' + '\n'.join(f'{i} => fixture_{i}(&expected).await?,' for i in range(len(fixtures))) + '\n_=>panic!("Unknown trusted fixture"),\n}\nOk(())\n}\n'
        (project / 'src/main.rs').write_text(source)
        wire.run(['cargo', 'build', '--locked'],project,build_env,timeout=900)
        assert (project / 'Cargo.lock').read_bytes() == (qa / 'Cargo.lock').read_bytes()
        (project / 'resolved-dependencies.txt').write_text(wire.run(['cargo','tree','--locked'],project,build_env))
        metadata = json.loads(wire.run(['cargo','metadata','--locked','--no-deps','--format-version','1'],project,build_env))
        binary = Path(metadata['target_directory']) / 'debug/moleapi-rust-snippet-qa'
        assert binary.is_file()
        tools['consumer_binary'] = wire.digest(binary)
        for index, (profile,root,manifest,saved,expected,snippet,original) in enumerate(fixtures):
            (root / 'upload.bin').write_bytes(wire.BYTES)
            (root / "upload-鼹鼠'quote.bin").write_bytes(wire.BYTES)
            wire.run([str(binary),str(index),'' if profile == 'head-binary' else '{"ok":true}'],root,env,timeout=30)
            record = records.get(timeout=10)
            (root / 'wire-evidence.json').write_text(json.dumps(record, indent=2) + '\n')
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
            item = {'client': 'reqwest', 'profile': profile, 'platform': platform.platform(), 'versions': versions,
                    'tool_sha256': tools, 'engine_sha256': wire.digest(wire.ROOT / 'vendor/snippet-engine/engine.js'),
                    'source_sha256': manifest['source_sha256'], 'snippet_sha256': manifest['files'][snippet.name], 'result': 'passed'}
            (root / 'runtime-evidence.json').write_text(json.dumps(item, indent=2) + '\n')
            evidence.append(item)
            print(f'PASS Rust/reqwest/{profile}: original syntax/native request and exact body', flush=True)
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
    parser.add_argument('--profiles', default='core,form,text,binary,binary-mime,multipart,dollar,json-values,json-scalar,json-null,json-empty,get-binary,head-binary,delete-json,text-json,multipart-meta,multipart-text,json-object,headers-quoted,json-controls,form-empty')
    verify(parser.parse_args())
