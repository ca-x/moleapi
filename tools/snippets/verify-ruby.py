#!/usr/bin/env python3
"""Syntax-check/run original trusted Ruby snippets against native HTTP."""
import argparse
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
    for path in (args.ruby, args.node, args.output):
        assert path.is_absolute()
    assert args.ruby.is_file() and args.node.is_file()
    args.output.mkdir(exist_ok=False)
    env = os.environ.copy()
    # The fixture owns its loopback endpoint; Ruby should not proxy it externally.
    for name in ('HTTP_PROXY', 'HTTPS_PROXY', 'http_proxy', 'https_proxy', 'ALL_PROXY', 'all_proxy'):
        env.pop(name, None)
    versions = {'ruby': wire.run([str(args.ruby), '--version'], args.output, env),
                'node': wire.run([str(args.node), '--version'], args.output, env)}
    tools = {'ruby': wire.digest(args.ruby), 'node': wire.digest(args.node)}
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
            assert profile in ('core', 'form', 'text', 'binary', 'binary-mime', 'multipart', 'dollar', 'interpolation')
            root = args.output / profile
            wire.run(['cargo', 'run', '--locked', '-p', 'moleapi-generation', '--example', 'fixture_snippets', '--', str(root),
                      f'http://127.0.0.1:{port}', 'ruby', profile], wire.ROOT, env)
            manifest = json.loads((root / 'fixture-manifest.json').read_text())
            assert manifest['target'] == 'ruby' and manifest['profile'] == profile
            assert wire.digest(root / 'fixture-request.json') == manifest['source_sha256']
            snippet = root / 'snippet.rb'
            original = snippet.read_bytes()
            assert wire.digest(snippet) == manifest['files']['snippet.rb']
            (root / 'upload.bin').write_bytes(wire.BYTES)
            # Original code remains intact. Only assertions and native IO cleanup follow.
            wrapper = root / 'consumer.rb'
            wrapper.write_text(original.decode() + '''
raise 'Fixture status failed' unless response.code == '200'
if defined?(form_data)
  form_data.each { |part| part[1].close if part[1].is_a?(IO) && !part[1].closed? }
end
''')
            wire.run([str(args.ruby), '-c', str(wrapper)], root, env)
            output = wire.run([str(args.ruby), str(wrapper)], root, env, timeout=30)
            assert '{"ok":true}' in output, output
            record = records.get(timeout=10)
            (root / 'wire-evidence.json').write_text(json.dumps(record, indent=2) + '\n')
            wire.check('core' if profile == 'interpolation' else 'text' if profile == 'dollar' else profile, record,
                       expected_text='literal $ and ${1 + 2} 鼹鼠' if profile == 'dollar' else None,
                       expected_json={'message': 'literal #{1 + 2} #@value #$value'} if profile == 'interpolation' else None)
            assert records.empty() and snippet.read_bytes() == original
            item = {'client': 'native', 'profile': profile, 'platform': platform.platform(), 'versions': versions,
                    'tool_sha256': tools, 'engine_sha256': wire.digest(wire.ROOT / 'vendor/snippet-engine/engine.js'),
                    'source_sha256': manifest['source_sha256'], 'snippet_sha256': manifest['files']['snippet.rb'], 'result': 'passed'}
            (root / 'runtime-evidence.json').write_text(json.dumps(item, indent=2) + '\n')
            evidence.append(item)
            print(f'PASS Ruby/native/{profile}: original syntax/native request and exact body', flush=True)
        (args.output / 'runtime-evidence.json').write_text(json.dumps(evidence, indent=2) + '\n')
    finally:
        server.terminate()
        server.wait(timeout=10)
        server.stdout.close()
        if 'receiver' in locals():
            receiver.join(timeout=2)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--ruby', type=Path, required=True)
    parser.add_argument('--node', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--profiles', default='core,form,text,binary,binary-mime,multipart,dollar,interpolation')
    verify(parser.parse_args())
