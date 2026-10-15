#!/usr/bin/env python3
"""Compile original trusted Kotlin snippets and observe native OkHttp requests."""
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

IMPORTS = '''import okhttp3.*
import okhttp3.MediaType.Companion.toMediaTypeOrNull
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.RequestBody.Companion.asRequestBody
import java.io.File
'''
BUILD = '''plugins {
    id 'org.jetbrains.kotlin.jvm' version '2.4.0'
    id 'application'
}
repositories { mavenCentral() }
dependencies { implementation 'com.squareup.okhttp3:okhttp:4.12.0' }
kotlin { jvmToolchain(21) }
application {
    mainClass = 'MainKt'
    applicationDefaultJvmArgs = ['-Xmx256m', '-XX:ActiveProcessorCount=2']
}
dependencyLocking { lockAllConfigurations() }
tasks.register('recordDependencies') {
    doLast {
        file('runtime-classpath.txt').text = configurations.runtimeClasspath.files.sort().collect { it.absolutePath }.join(System.lineSeparator())
    }
}
'''


def verify(args):
    for path in (args.jdk, args.gradle, args.node, args.output):
        assert path.is_absolute()
    assert (args.jdk / 'bin/java').is_file() and args.gradle.is_file() and args.node.is_file()
    args.output.mkdir(exist_ok=False)
    env = {**os.environ, 'JAVA_HOME': str(args.jdk), 'PATH': str(args.jdk / 'bin') + os.pathsep + os.environ['PATH'],
           'GRADLE_OPTS': '-Xmx512m -XX:ActiveProcessorCount=2'}
    for name in ('JAVA_TOOL_OPTIONS', 'JDK_JAVA_OPTIONS', '_JAVA_OPTIONS'):
        env.pop(name, None)
    versions = {'java': wire.run([str(args.jdk / 'bin/java'), '--version'], args.output, env),
                'gradle': wire.run([str(args.gradle), '--version'], args.output, env),
                'node': wire.run([str(args.node), '--version'], args.output, env)}
    tools = {key: wire.digest(path) for key, path in {'java': args.jdk / 'bin/java', 'gradle': args.gradle, 'node': args.node}.items()}
    server_file = args.output / 'server.cjs'
    server_file.write_text("""const http=require('http');const server=http.createServer((request,response)=>{const chunks=[];request.on('data',part=>chunks.push(part));request.on('end',()=>{console.log(JSON.stringify({method:request.method,path:request.url,headers:request.headers,body:Buffer.concat(chunks).toString('base64')}));response.writeHead(200,{'Content-Type':'application/json'});response.end('{"ok":true}');});});server.listen(0,'127.0.0.1',()=>console.log(JSON.stringify({port:server.address().port})));""")
    records = queue.Queue()
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
                      f'http://127.0.0.1:{port}', 'kotlin', profile], wire.ROOT, env)
            manifest = json.loads((root / 'fixture-manifest.json').read_text())
            assert manifest['target'] == 'kotlin' and manifest['profile'] == profile
            assert wire.digest(root / 'fixture-request.json') == manifest['source_sha256']
            snippet = root / 'snippet.kt'
            original = snippet.read_bytes()
            assert wire.digest(snippet) == manifest['files']['snippet.kt']
            project = root / 'consumer'
            (project / 'src/main/kotlin').mkdir(parents=True)
            (project / 'upload.bin').write_bytes(wire.BYTES)
            # Imports/main and cleanup only; original emitted code is not rewritten.
            (project / 'src/main/kotlin/Main.kt').write_text(IMPORTS + '\nfun main() {\n' + original.decode() + '''
check(response.code == 200)
println(response.body!!.string())
response.close()
client.connectionPool.evictAll()
client.dispatcher.executorService.shutdown()
}
''')
            (project / 'settings.gradle').write_text("rootProject.name = 'native-kotlin-snippet'\n")
            (project / 'build.gradle').write_text(BUILD)
            output = wire.run([str(args.gradle), '--no-daemon', '--max-workers=2', '--console=plain', '--write-locks',
                               'run', 'recordDependencies'], project, env, timeout=600)
            assert '{"ok":true}' in output, output
            record = records.get(timeout=10)
            (project / 'wire-evidence.json').write_text(json.dumps(record, indent=2) + '\n')
            wire.check('text' if profile == 'dollar' else profile, record,
                       expected_text='literal $ and ${1 + 2} 鼹鼠' if profile == 'dollar' else None)
            assert records.empty() and snippet.read_bytes() == original
            jars = (project / 'runtime-classpath.txt').read_text().splitlines()
            item = {'client': 'okhttp', 'profile': profile, 'platform': platform.platform(),
                    'versions': versions, 'tool_sha256': tools, 'engine_sha256': wire.digest(wire.ROOT / 'vendor/snippet-engine/engine.js'),
                    'source_sha256': manifest['source_sha256'], 'snippet_sha256': manifest['files']['snippet.kt'],
                    'dependency_sha256': {Path(path).name: wire.digest(Path(path)) for path in jars if Path(path).is_file()}, 'result': 'passed'}
            (project / 'runtime-evidence.json').write_text(json.dumps(item, indent=2) + '\n')
            evidence.append(item)
            print(f'PASS Kotlin/OkHttp/{profile}: original native request and exact body bytes', flush=True)
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
    parser.add_argument('--gradle', type=Path, required=True)
    parser.add_argument('--node', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--profiles', default='core,form,text,binary,binary-mime,multipart,dollar')
    verify(parser.parse_args())
