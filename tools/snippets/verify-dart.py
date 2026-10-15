#!/usr/bin/env python3
"""Analyze/AOT-run original trusted Dart snippets against native HTTP fixtures."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import platform
import queue
import re
import subprocess
import threading

spec = importlib.util.spec_from_file_location('java_wire_qa', Path(__file__).with_name('verify-java.py'))
wire = importlib.util.module_from_spec(spec)
spec.loader.exec_module(wire)

PACKAGES = {'http': '1.6.0', 'dio': '5.11.1', 'http_parser': '4.1.2'}


def verify(args):
    for path in (args.dart, args.node, args.output):
        assert path.is_absolute()
    assert args.dart.is_file() and args.node.is_file()
    args.output.mkdir(exist_ok=False)
    env = {**os.environ, 'PUB_CACHE': str(args.output / 'pub-cache')}
    version = subprocess.run([str(args.dart), '--version'], capture_output=True, text=True, check=True)
    versions = {'dart': version.stdout + version.stderr, 'node': wire.run([str(args.node), '--version'], args.output, env)}
    tools = {'dart': wire.digest(args.dart), 'node': wire.digest(args.node)}
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
            assert profile in ('core', 'form', 'text', 'binary', 'binary-mime', 'multipart', 'dollar')
            root = args.output / profile
            wire.run(['cargo', 'run', '--locked', '-p', 'moleapi-generation', '--example', 'fixture_snippets', '--', str(root),
                      f'http://127.0.0.1:{port}', 'dart', profile], wire.ROOT, env)
            manifest = json.loads((root / 'fixture-manifest.json').read_text())
            assert manifest['target'] == 'dart' and manifest['profile'] == profile
            assert wire.digest(root / 'fixture-request.json') == manifest['source_sha256']
            for client in args.clients.split(','):
                assert client in ('http', 'dio')
                original_file = root / (client + '.dart')
                original = original_file.read_bytes()
                assert wire.digest(original_file) == manifest['files'][original_file.name]
                project = root / client
                (project / 'bin').mkdir(parents=True)
                (project / 'upload.bin').write_bytes(wire.BYTES)
                code = original.decode()
                if 'void main()' in code or 'Future<void> main()' in code:
                    program = code
                else:
                    imports = ["import 'package:dio/dio.dart';" if client == 'dio' else "import 'package:http/http.dart' as http;"]
                    if 'json.' in code or 'utf8.' in code:
                        imports.append("import 'dart:convert';")
                    if re.search(r'(?<![\w.])File\(', code):
                        imports.append("import 'dart:io';")
                    if 'MediaType(' in code or 'MediaType.parse' in code:
                        imports.append("import 'package:http_parser/http_parser.dart';")
                    program = '\n'.join(imports) + '\nFuture<void> main() async {\n' + code + ('\ndio.close(force: true);' if client == 'dio' else '') + '\n}\n'
                # Full original programs stay unchanged; bare snippets get imports/main only.
                (project / 'bin/main.dart').write_text(program)
                (project / 'pubspec.yaml').write_text('name: native_dart_snippet\npublish_to: none\nenvironment:\n  sdk: ">=3.8.0 <4.0.0"\ndependencies:\n' + ''.join(f'  {name}: {version}\n' for name, version in PACKAGES.items()))
                wire.run([str(args.dart), 'pub', 'get'], project, env)
                wire.run([str(args.dart), 'analyze', '--fatal-infos'], project, env)
                executable = project / 'native-snippet'
                wire.run([str(args.dart), 'compile', 'exe', 'bin/main.dart', '-o', str(executable)], project, env)
                output = wire.run([str(executable)], project, env, timeout=30)
                assert '{"ok":true}' in output, output
                record = records.get(timeout=10)
                (project / 'wire-evidence.json').write_text(json.dumps(record, indent=2) + '\n')
                wire.check('text' if profile == 'dollar' else profile, record,
                           expected_text='literal $ and ${1 + 2} 鼹鼠' if profile == 'dollar' else None)
                assert records.empty() and original_file.read_bytes() == original
                lock = project / 'pubspec.lock'
                item = {'client': client, 'profile': profile, 'platform': platform.platform(), 'versions': versions,
                        'packages': PACKAGES, 'tool_sha256': tools, 'engine_sha256': wire.digest(wire.ROOT / 'vendor/snippet-engine/engine.js'),
                        'source_sha256': manifest['source_sha256'], 'snippet_sha256': manifest['files'][original_file.name],
                        'pub_lock_sha256': wire.digest(lock), 'aot_sha256': wire.digest(executable), 'result': 'passed'}
                (project / 'runtime-evidence.json').write_text(json.dumps(item, indent=2) + '\n')
                evidence.append(item)
                print(f'PASS Dart/{client}/{profile}: original analyzed/AOT native code and exact wire bytes', flush=True)
        (args.output / 'runtime-evidence.json').write_text(json.dumps(evidence, indent=2) + '\n')
    finally:
        server.terminate()
        server.wait(timeout=10)
        server.stdout.close()
        if 'receiver' in locals():
            receiver.join(timeout=2)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dart', type=Path, required=True)
    parser.add_argument('--node', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--profiles', default='core,form,text,binary,binary-mime,multipart,dollar')
    parser.add_argument('--clients', default='http,dio')
    verify(parser.parse_args())
