"""Compile/run trusted C# snippets against a local HTTP fixture.

Requires an installed .NET SDK supporting net10.0/C#11 and NuGet access/cache for
RestSharp114. No imported user request is ever executed. Run from repository root:
python3 tools/snippets/verify-csharp.py --dotnet /absolute/path/to/dotnet
"""
import argparse
import http.server
import json
import os
import pathlib
import queue
import subprocess
import tempfile
import threading
import urllib.parse

ROOT = pathlib.Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser()
parser.add_argument('--dotnet', required=True, type=pathlib.Path)
args = parser.parse_args()
if not args.dotnet.is_absolute() or not args.dotnet.is_file():
    parser.error('dotnet must be an explicitly selected absolute executable')
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

def run(command, cwd, env=None):
    result = subprocess.run(command, cwd=cwd, env=env, timeout=180, capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(f'{command}: {result.stdout}\n{result.stderr}')

server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Fixture)
threading.Thread(target=server.serve_forever, daemon=True).start()
try:
    with tempfile.TemporaryDirectory(prefix='moleapi-csharp-snippets-') as directory:
        output = pathlib.Path(directory)
        run(['cargo', 'run', '-p', 'moleapi-generation', '--example', 'fixture_snippets', '--locked', '--', str(output), f'http://127.0.0.1:{server.server_port}', 'csharp'], ROOT)
        env = {**os.environ, 'DOTNET_CLI_HOME': str(output / 'home'), 'DOTNET_CLI_TELEMETRY_OPTOUT': '1', 'DOTNET_SKIP_FIRST_TIME_EXPERIENCE': '1'}
        for client in ('httpclient', 'restsharp'):
            project = output / client
            project.mkdir()
            dependency = '<ItemGroup><PackageReference Include="RestSharp" Version="114.0.0" /></ItemGroup>' if client == 'restsharp' else ''
            (project / 'fixture.csproj').write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0</TargetFramework><ImplicitUsings>enable</ImplicitUsings><Nullable>enable</Nullable><LangVersion>11</LangVersion></PropertyGroup>' + dependency + '</Project>')
            imports = 'using System.Net;\nusing System.Net.Http.Headers;\n' + ('using RestSharp;\n' if client == 'restsharp' else '')
            snippet = (output / f'{client}.cs').read_text()
            (project / 'Program.cs').write_text(imports + snippet + '\nif(response.StatusCode != HttpStatusCode.OK) throw new Exception("Fixture request failed");\n')
            run([str(args.dotnet), 'run', '--project', str(project / 'fixture.csproj'), '--verbosity', 'quiet'], project, env)
            path, headers, body = records.get(timeout=10)
            headers = {key.lower(): value for key, value in headers.items()}
            assert headers['authorization'] == 'Bearer fixture-token', (client, headers)
            assert headers['x-test'] == 'moleapi-fixture', (client, headers)
            assert headers['content-type'].startswith('application/json'), (client, headers)
            assert urllib.parse.parse_qs(urllib.parse.urlsplit(path).query) == {'existing': ['one'], 'q': ['space & +']}, (client, path)
            assert json.loads(body) == {'message': '鼹鼠 "quotes" & +'}, (client, body)
            print(f'PASS C#/{client}: actual POST/auth/header/query/Unicode JSON')
        assert records.empty(), 'A snippet sent unexpected additional requests'
finally:
    server.shutdown()
    server.server_close()
