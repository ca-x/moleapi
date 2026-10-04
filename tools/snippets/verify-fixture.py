"""Explicitly execute trusted generated fixture code, never imported user requests.
Run from repository root: python3 tools/snippets/verify-fixture.py
Needs Node, Python, Go, C compiler/libcurl, Rust and cached reqwest dependencies.
Outputs and the fixture Rust wrapper are isolated in a temporary directory.
"""
import http.server
import json
import pathlib
import queue
import subprocess
import tempfile
import threading
import urllib.parse

ROOT = pathlib.Path(__file__).resolve().parents[2]
records = queue.Queue()
class Fixture(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        body = self.rfile.read(int(self.headers.get('Content-Length', 0)))
        records.put((self.path, dict(self.headers), body))
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.end_headers()
        self.wfile.write(b'{"ok":true}')
    def log_message(self, *args):
        pass

def run(command, cwd=ROOT, timeout=180):
    result = subprocess.run(command, cwd=cwd, timeout=timeout, capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(f'{command}: {result.stdout}\n{result.stderr}')
    return result

def verify(label, command, cwd=ROOT):
    run(command, cwd)
    path, headers, body = records.get(timeout=10)
    headers = {key.lower(): value for key, value in headers.items()}
    assert headers['x-test'] == 'moleapi-fixture', (label, headers)
    assert headers['authorization'] == 'Bearer fixture-token', (label, headers)
    assert headers['content-type'].startswith('application/json'), (label, headers)
    assert urllib.parse.parse_qs(urllib.parse.urlsplit(path).query) == {'existing':['one'],'q':['space & +']}, (label, path)
    assert json.loads(body) == {'message':'鼹鼠 "quotes" & +'}, (label, body)
    print(f'PASS {label}: actual POST/auth/header/query/Unicode JSON')

server=http.server.ThreadingHTTPServer(('127.0.0.1',0), Fixture)
threading.Thread(target=server.serve_forever,daemon=True).start()
try:
    with tempfile.TemporaryDirectory(prefix='moleapi-snippets-') as temporary:
        output=pathlib.Path(temporary)
        run(['cargo','run','-p','moleapi-generation','--example','fixture_snippets','--locked','--',str(output),f'http://127.0.0.1:{server.server_port}'])
        verify('Node/fetch',['node',str(output/'node.mjs')])
        verify('JavaScript/fetch on Node fetch runtime',['node',str(output/'browser.mjs')])
        verify('Python/http.client',['python3',str(output/'python.py')])
        verify('Go/net-http',['go','run',str(output/'main.go')])
        verify('Shell/cURL',['sh',str(output/'curl.sh')])
        run(['cc',str(output/'c.c'),'-lcurl','-o',str(output/'c-fixture')])
        verify('C/libcurl',[str(output/'c-fixture')])
        rust=output/'rust';(rust/'src').mkdir(parents=True)
        (rust/'Cargo.toml').write_text('[package]\nname="moleapi-snippet-fixture"\nversion="0.0.0"\nedition="2024"\n[dependencies]\nreqwest={version="0.12",default-features=false,features=["json","rustls-tls"]}\ntokio={version="1",features=["macros","rt-multi-thread"]}\nserde_json="1"\n')
        # The upstream Rust snippet belongs inside an async function, not a complete app.
        (rust/'src/main.rs').write_text('#[tokio::main]\nasync fn main()->Result<(),Box<dyn std::error::Error>>{\n'+(output/'rust.rs').read_text()+'\nassert!(response.status().is_success());\nOk(())\n}\n')
        verify('Rust/reqwest (async fixture wrapper)',['cargo','run','--offline','--quiet'],rust)
        assert records.empty(), 'A generated example sent unexpected additional requests'
finally:
    server.shutdown();server.server_close()
