#!/usr/bin/env python3
"""Run a packaged JVM and real project API with external Java configuration cleared."""
import argparse
import json
import os
from pathlib import Path
import queue
import sys
import re
import subprocess
import tempfile
import threading
import urllib.request
import uuid
from prepare_java_runtime import ROOT, host_target, verify_payload


def api(base, path, body=None, token=None):
    headers = {'content-type': 'application/json'}
    if token:
        headers['authorization'] = 'Bearer ' + token
    request = urllib.request.Request(base + path, headers=headers,
                                     data=None if body is None else json.dumps(body).encode())
    with urllib.request.urlopen(request, timeout=90) as response:
        return json.load(response)


def generate(base, setup):
    suffix = uuid.uuid4().hex
    owner = api(base, '/api/auth/register', {'username': 'bundle-' + suffix[:16],
                                           'password': uuid.uuid4().hex, 'setup_token': setup})['token']
    catalog = api(base, '/api/generation/projects/catalog', token=owner)
    assert catalog['java_available'] is True, 'Bundled JVM was not selected by the actual server'
    source = json.dumps({'openapi': '3.1.0', 'info': {'title': 'Bundled Java fixture', 'version': '1'},
                         'paths': {}, 'components': {'schemas': {'Pet': {'type': 'object', 'required': ['id'],
                                                                         'properties': {'id': {'type': 'integer'}}}}}})
    workspace = 'bundle-' + suffix
    data = {'schema_version': 1, 'collections': [], 'environments': [], 'active_environment_id': None,
            'specifications': [{'id': 'source', 'name': 'Bundled Java', 'kind': 'openapi', 'dialect': '3.1.0', 'source': source}]}
    api(base, '/api/workspaces', {'id': workspace, 'name': 'Bundled JVM smoke', 'data': data}, owner)
    artifact = api(base, '/api/generation/projects', {'workspace_id': workspace, 'specification_id': 'source',
                                                     'job_id': suffix, 'target': 'go', 'options': {}}, owner)
    assert artifact['engine'] == 'openapi-generator@7.26.0'
    assert any(file['path'] == 'model_pet.go' and 'type Pet struct' in file['content'] for file in artifact['files'])
    saved = api(base, '/api/workspaces/' + workspace, token=owner)
    assert saved['data']['specifications'][0]['source'] == source
    manifest = next(file for file in artifact['files'] if file['path'] == 'moleapi-generation.json')
    assert json.loads(manifest['content'])['source_sha256'] == artifact['source_sha256']
    return {'engine': artifact['engine'], 'files': len(artifact['files']), 'source_sha256': artifact['source_sha256']}


def main(args):
    process = None
    with tempfile.TemporaryDirectory(prefix='moleapi-bundled-java-') as temporary:
        if args.container:
            assert os.environ.get('GITHUB_ACTIONS') == 'true', 'Container verification requires the explicit CI fixture'
            version = subprocess.run(['docker', 'exec', args.container, 'env', '-i',
                                      '/opt/moleapi/runtime/java/payload/bin/java', '-version'],
                                     capture_output=True, text=True, timeout=15, check=True)
            base = args.url
            if not base:
                raise ValueError('Provide the owned container fixture URL')
        else:
            binary = args.binary.resolve()
            if os.name == 'nt':
                binary = binary.with_suffix('.exe')
            assert binary.is_file()
            manifest = json.loads((ROOT / 'vendor/java-runtime/manifest.json').read_text())
            target = host_target()
            resource_root = args.resources.resolve() if args.resources else binary.parent
            if args.runtime_only and sys.platform == 'darwin' and not args.resources:
                applications = list(binary.parent.glob('bundle/macos/*.app/Contents/Resources'))
                if len(applications) != 1:
                    raise ValueError('Expected exactly one packaged macOS application resource directory')
                resource_root = applications[0]
            resources = resource_root / 'runtime/java'
            verify_payload(resources, manifest, target)
            jvm = resources / manifest['assets'][target]['java']
            cleared = {key: os.environ[key] for key in ('SYSTEMROOT',) if key in os.environ}
            version = subprocess.run([str(jvm), '-version'], env=cleared,
                                     capture_output=True, text=True, timeout=15, check=True)
            assert 'version "21.0.12.1"' in version.stdout + version.stderr
            if args.runtime_only:
                print(f'PASS bundled Java resources: {target}; original inventory, licenses and actual JVM')
                return
            environment = {key: value for key, value in os.environ.items()
                           if key not in ('MOLEAPI_CODEGEN_JAVA', 'JAVA_HOME', 'JDK_JAVA_OPTIONS', 'JAVA_TOOL_OPTIONS', '_JAVA_OPTIONS',
                                          'MOLEAPI_DATABASE_URL', 'MOLEAPI_BIND', 'MOLEAPI_SETUP_TOKEN')}
            # An invalid PATH makes accidental ambient Java discovery observable.
            environment['PATH'] = str(Path(temporary) / 'no-executables')
            with (Path(temporary) / 'server.log').open('w') as errors:
                process = subprocess.Popen([str(binary), '--bind', '127.0.0.1:0', '--database', str(Path(temporary) / 'server.db'),
                                            '--setup-token', args.setup_token], env=environment, stdout=subprocess.PIPE,
                                           stderr=errors, text=True)
                lines = queue.Queue()
                def read():
                    for line in process.stdout:
                        if match := re.search(r'MoleAPI listening on (http://127\.0\.0\.1:\d+)', line):
                            lines.put(match.group(1))
                reader = threading.Thread(target=read, daemon=True)
                reader.start()
                try:
                    base = lines.get(timeout=30)
                    details = generate(base, args.setup_token)
                finally:
                    process.terminate()
                    try:
                        process.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=10)
                    process.stdout.close()
                    reader.join(timeout=2)
            details['java_version'] = (version.stdout + version.stderr).strip()
            print('PASS bundled Java: actual packaged server generated original Go code with no external Java or PATH')
            print(json.dumps(details, ensure_ascii=False))
            return
        details = generate(base, args.setup_token)
        assert 'version "21.0.12.1"' in version.stdout + version.stderr
        details['java_version'] = (version.stdout + version.stderr).strip()
        print('PASS bundled Java: actual container JVM and owned generation API')
        print(json.dumps(details, ensure_ascii=False))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', nargs='?', type=Path)
    parser.add_argument('--container')
    parser.add_argument('--url')
    parser.add_argument('--setup-token', default='bundled-java-qa-only')
    parser.add_argument('--resources', type=Path, help='Explicit owned application resource directory')
    parser.add_argument('--runtime-only', action='store_true', help='Verify native desktop resources and JVM without launching a server')
    args = parser.parse_args()
    if not args.binary and not args.container:
        parser.error('Provide an owned packaged binary or CI container fixture')
    main(args)
