"""Run trusted new Postman adapters, including chunked bodies, against Node HTTP.

Requires Node and developer-installed request2.88.2/unirest0.6.0/follow-redirects1.16.1:
python3 tools/snippets/verify-postman-adapters.py --node-modules /absolute/path/node_modules
No user request or imported code is executed.
"""
import argparse
import base64
import json
import os
from pathlib import Path
import queue
import subprocess
import tempfile
import threading
import urllib.parse

ROOT = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser()
parser.add_argument('--node-modules', required=True, type=Path)
args = parser.parse_args()
if not args.node_modules.is_absolute() or not args.node_modules.is_dir():
    parser.error('select an absolute directory of installed target dependencies')
records = queue.Queue()

def run(command, env=None):
    result = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True, timeout=180)
    if result.returncode:
        raise RuntimeError(result.stdout + result.stderr)

with tempfile.TemporaryDirectory(prefix='moleapi-postman-adapters-') as directory:
    output = Path(directory)
    fixture = output / 'server.cjs'
    fixture.write_text("""const http=require('http');const server=http.createServer((request,response)=>{const chunks=[];request.on('data',chunk=>chunks.push(chunk));request.on('end',()=>{console.log(JSON.stringify({method:request.method,path:request.url,headers:request.headers,body:Buffer.concat(chunks).toString('base64')}));response.writeHead(200,{'Content-Type':'application/json'});response.end('{"ok":true}');});});server.listen(0,'127.0.0.1',()=>console.log(JSON.stringify({port:server.address().port})));""")
    server = subprocess.Popen(['node', str(fixture)], stdout=subprocess.PIPE, text=True)
    try:
        port = json.loads(server.stdout.readline())['port']
        def receive():
            for line in server.stdout:
                data = json.loads(line)
                records.put((data['method'], data['path'], data['headers'], base64.b64decode(data['body'])))
        threading.Thread(target=receive, daemon=True).start()
        run(['cargo', 'run', '-p', 'moleapi-generation', '--example', 'fixture_snippets', '--locked', '--', str(output), f'http://127.0.0.1:{port}', 'parity'])
        for client in ['native', 'request', 'unirest']:
            run(['node', str(output / (client + '.cjs'))], {**os.environ, 'NODE_PATH': str(args.node_modules)})
            method, path, headers, body = records.get(timeout=10)
            assert method == 'POST', method
            headers = {key.lower(): value for key, value in headers.items()}
            assert headers['authorization'] == 'Bearer fixture-token', headers
            assert headers['x-test'] == 'moleapi-fixture', headers
            assert headers['content-type'].startswith('application/json'), headers
            assert urllib.parse.parse_qs(urllib.parse.urlsplit(path).query) == {'existing': ['one'], 'q': ['space & +']}, path
            assert json.loads(body) == {'message': '鼹鼠 "quotes" & +'}, body
            print(f'PASS {client}: real POST/auth/headers/query/Unicode JSON')
        assert records.empty()
    finally:
        server.terminate()
        server.wait(timeout=10)
