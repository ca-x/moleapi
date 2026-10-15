#!/usr/bin/env python3
"""Compile original trusted Java snippets and observe real HTTP, never imported code."""
import argparse
import base64
from email import policy
from email.parser import BytesParser
import hashlib
import json
import os
from pathlib import Path
import platform
import queue
import subprocess
import threading
import urllib.parse

ROOT = Path(__file__).resolve().parents[2]
LIBRARIES = {
    'nethttp': (None, 'import java.net.http.*;', ''),
    'okhttp': ('com.squareup.okhttp3:okhttp:4.12.0', 'import okhttp3.*;',
               'client.connectionPool().evictAll(); client.dispatcher().executorService().shutdown();'),
    'asynchttp': ('org.asynchttpclient:async-http-client:3.0.2', 'import org.asynchttpclient.*;', ''),
    'unirest': ('com.konghq:unirest-java:3.14.5', 'import kong.unirest.*;', 'Unirest.shutDown();'),
}
BYTES = bytes([0, 255, 128, 10, 88])
NOTE = '鼹鼠 "quotes" & +'


def digest(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def run(command, cwd, env, timeout=300):
    print('RUN', ' '.join(map(str, command)), flush=True)
    result = subprocess.run(command, cwd=cwd, env=env, capture_output=True, text=True, timeout=timeout)
    if result.returncode:
        raise RuntimeError(result.stdout + '\n' + result.stderr)
    return result.stdout


def check(profile, record):
    assert record['method'] == 'POST', record
    headers = record['headers']
    assert headers['authorization'] == 'Bearer fixture-token'
    assert headers['x-test'] == 'moleapi-fixture'
    assert 'wrong.invalid' not in headers['host']
    assert urllib.parse.parse_qs(urllib.parse.urlsplit(record['path']).query) == {'existing': ['one'], 'q': ['space & +']}
    body = base64.b64decode(record['body'])
    mime = headers['content-type']
    if profile == 'core':
        assert mime.startswith('application/json') and json.loads(body) == {'message': NOTE}
    elif profile == 'form':
        assert mime.startswith('application/x-www-form-urlencoded')
        assert urllib.parse.parse_qs(body.decode(), keep_blank_values=True) == {'label': [NOTE, 'second'], 'empty': ['']}
    elif profile == 'text':
        assert mime.startswith('text/plain') and body == (NOTE + '\r\nline\0\vend').encode()
    elif profile in ('binary', 'binary-mime'):
        expected = 'application/vnd.moleapi.fixture' if profile == 'binary-mime' else 'application/octet-stream'
        assert mime.startswith(expected) and body == BYTES
    else:
        assert mime.startswith('multipart/form-data;')
        message = BytesParser(policy=policy.default).parsebytes(b'Content-Type: ' + mime.encode() + b'\r\nMIME-Version: 1.0\r\n\r\n' + body)
        assert message.is_multipart()
        parts = list(message.iter_parts())
        assert [part.get_param('name', header='Content-Disposition') for part in parts] == ['label', 'label', 'upload']
        assert parts[0].get_payload(decode=True).decode() == "鼹鼠 'quotes' & +"
        assert parts[1].get_payload(decode=True) == b'second'
        assert parts[2].get_filename() == 'upload.bin' and parts[2].get_content_type() == 'application/octet-stream'
        assert parts[2].get_payload(decode=True) == BYTES
        assert b'must-not-send' not in body


def verify(args):
    for path in (args.jdk, args.maven, args.node, args.output):
        assert path.is_absolute()
    assert (args.jdk / 'bin/java').is_file() and (args.jdk / 'bin/javac').is_file()
    assert args.maven.is_file() and args.node.is_file()
    args.output.mkdir(exist_ok=False)
    env = {**os.environ, 'JAVA_HOME': str(args.jdk), 'PATH': str(args.jdk / 'bin') + os.pathsep + os.environ['PATH']}
    env.pop('JAVA_TOOL_OPTIONS', None)
    env.pop('JDK_JAVA_OPTIONS', None)
    env.pop('_JAVA_OPTIONS', None)
    versions = {'java': run([str(args.jdk / 'bin/java'), '--version'], args.output, env),
                'maven': run([str(args.maven), '--version'], args.output, env),
                'node': run([str(args.node), '--version'], args.output, env)}
    tools = {name: digest(path) for name, path in {'java': args.jdk / 'bin/java',
              'javac': args.jdk / 'bin/javac', 'maven': args.maven, 'node': args.node}.items()}
    engine = digest(ROOT / 'vendor/snippet-engine/engine.js')
    records = queue.Queue()
    server_file = args.output / 'server.cjs'
    server_file.write_text("""const http=require('http'); const server=http.createServer((request,response)=>{const chunks=[];request.on('data',part=>chunks.push(part));request.on('end',()=>{console.log(JSON.stringify({method:request.method,path:request.url,headers:request.headers,body:Buffer.concat(chunks).toString('base64')}));response.writeHead(200,{'Content-Type':'application/json'});response.end('{"ok":true}');});});server.listen(0,'127.0.0.1',()=>console.log(JSON.stringify({port:server.address().port})));""")
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
            assert profile in ('core', 'form', 'text', 'binary', 'binary-mime', 'multipart')
            root = args.output / profile
            run(['cargo', 'run', '--locked', '-p', 'moleapi-generation', '--example', 'fixture_snippets', '--',
                 str(root), f'http://127.0.0.1:{port}', 'java', profile], ROOT, env)
            manifest = json.loads((root / 'fixture-manifest.json').read_text())
            source = (root / 'fixture-request.json').read_bytes()
            assert manifest['target'] == 'java' and manifest['profile'] == profile
            assert hashlib.sha256(source).hexdigest() == manifest['source_sha256']
            for client, (dependency, imports, cleanup) in LIBRARIES.items():
                snippet = root / (client + '.java')
                original = snippet.read_bytes()
                assert hashlib.sha256(original).hexdigest() == manifest['files'][snippet.name]
                project = root / client
                (project / 'src/main/java').mkdir(parents=True)
                (project / 'upload.bin').write_bytes(BYTES)
                code = original.decode()
                # Only the host main/imports and resource cleanup are supplied here.
                # The emitted Java source stays byte-identical inside the wrapper.
                if client == 'unirest':
                    body = 'try {\n' + code + '\n} finally { ' + cleanup + ' }'
                else:
                    body = code + '\n' + cleanup
                (project / 'src/main/java/Main.java').write_text(imports + '\npublic class Main { public static void main(String[] args) throws Exception {\n' + body + '\n} }\n')
                dependencies = ''
                if dependency:
                    group, artifact, version = dependency.split(':')
                    dependencies = f'<dependencies><dependency><groupId>{group}</groupId><artifactId>{artifact}</artifactId><version>{version}</version></dependency></dependencies>'
                (project / 'pom.xml').write_text('<project xmlns="http://maven.apache.org/POM/4.0.0"><modelVersion>4.0.0</modelVersion><groupId>moleapi.qa</groupId><artifactId>java-snippet</artifactId><version>1</version><properties><maven.compiler.release>21</maven.compiler.release><project.build.sourceEncoding>UTF-8</project.build.sourceEncoding></properties>' + dependencies + '<build><plugins><plugin><groupId>org.apache.maven.plugins</groupId><artifactId>maven-compiler-plugin</artifactId><version>3.14.0</version></plugin></plugins></build></project>')
                run([str(args.maven), '-B', '-q', 'compile', 'org.apache.maven.plugins:maven-dependency-plugin:3.8.1:build-classpath', '-Dmdep.outputFile=classpath.txt'], project, env)
                extra = (project / 'classpath.txt').read_text().strip() if (project / 'classpath.txt').exists() else ''
                assert not dependency or extra, 'Pinned library classpath missing'
                classpath = str(project / 'target/classes') + os.pathsep + extra
                output = run([str(args.jdk / 'bin/java'), '-Xmx256m', '-XX:ActiveProcessorCount=2', '-cp', classpath, 'Main'], project, env, timeout=30)
                assert any(line.strip() == '{"ok":true}' for line in output.splitlines()), output
                record = records.get(timeout=10)
                (project / 'wire-evidence.json').write_text(json.dumps(record, indent=2) + '\n')
                check(profile, record)
                assert records.empty(), 'Unexpected extra requests'
                assert snippet.read_bytes() == original
                run([str(args.maven), '-B', '-q', 'org.apache.maven.plugins:maven-dependency-plugin:3.8.1:tree', '-DoutputFile=resolved-dependencies.txt'], project, env)
                item = {'client': client, 'profile': profile, 'dependency': dependency or 'JDK21',
                        'snippet_sha256': manifest['files'][snippet.name], 'source_sha256': manifest['source_sha256'],
                        'engine_sha256': engine, 'tool_sha256': tools,
                        'dependency_sha256': {Path(path).name: digest(Path(path)) for path in extra.split(os.pathsep) if path},
                        'platform': platform.platform(), 'tool_versions': versions, 'result': 'passed'}
                (project / 'runtime-evidence.json').write_text(json.dumps(item, indent=2) + '\n')
                evidence.append(item)
                print(f'PASS Java/{client}/{profile}: actual HTTP/auth/query/header/body bytes', flush=True)
        (args.output / 'runtime-evidence.json').write_text(json.dumps(evidence, indent=2) + '\n')
    finally:
        server.terminate()
        server.wait(timeout=10)
        server.stdout.close()
        if 'receiver' in locals():
            receiver.join(timeout=2)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jdk', type=Path, required=True)
    parser.add_argument('--maven', type=Path, required=True)
    parser.add_argument('--node', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--profiles', default='core,form,text,binary,binary-mime,multipart')
    verify(parser.parse_args())
