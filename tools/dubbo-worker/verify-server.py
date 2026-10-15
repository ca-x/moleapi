#!/usr/bin/env python3
"""Exercise the real hosted API through the checked classic Dubbo worker."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import selectors
import subprocess
import tempfile
import time
import urllib.error
import urllib.request


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--server', type=Path, required=True)
    parser.add_argument('--java', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--provider-port', type=int, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    for path in (args.server, args.java, args.jar):
        assert path.is_absolute()
    assert args.server.is_file() and args.java.is_file() and args.jar.is_file()
    with tempfile.TemporaryDirectory(prefix='moleapi-dubbo-hosted-') as directory:
        root = Path(directory)
        environment = {**os.environ, 'MOLEAPI_DUBBO_WORKER_JAR': str(args.jar),
                       'MOLEAPI_CODEGEN_JAVA': str(args.java),
                       'MOLEAPI_SETUP_TOKEN': 'dubbo-qa-setup-token',
                       'MOLEAPI_ALLOW_PRIVATE_NETWORK': 'true',
                       'MOLEAPI_ALLOW_REGISTRATION': 'true'}
        server = subprocess.Popen([str(args.server), '--database', str(root / 'hosted.db'),
                                   '--bind', '127.0.0.1:0'], env=environment,
                                  stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                  text=True, bufsize=1)
        try:
            watcher = selectors.DefaultSelector()
            watcher.register(server.stdout, selectors.EVENT_READ)
            deadline = time.monotonic() + 90
            base = None
            while time.monotonic() < deadline and base is None:
                if server.poll() is not None:
                    raise RuntimeError(f'hosted server exited {server.returncode}')
                if watcher.select(timeout=1):
                    line = server.stdout.readline()
                    if line.startswith('MoleAPI listening on '):
                        base = line.strip().split(' on ', 1)[1]
            assert base, 'hosted server did not start'

            def call(method, path, body=None, token=None):
                payload = None if body is None else json.dumps(body, ensure_ascii=False).encode()
                headers = {'content-type': 'application/json'}
                if token: headers['authorization'] = f'Bearer {token}'
                request = urllib.request.Request(base + path, data=payload, headers=headers, method=method)
                try:
                    with urllib.request.urlopen(request, timeout=30) as response:
                        return response.status, json.load(response)
                except urllib.error.HTTPError as error:
                    return error.code, json.load(error)

            def register(name):
                status, value = call('POST', '/api/auth/register', {'username':name,
                    'password':'goodpassword123','setup_token':'dubbo-qa-setup-token'})
                assert status == 200, value
                return value['token']

            owner = register('dubbo-owner')
            stranger = register('dubbo-stranger')
            source = {
                'id':'r', 'name':'Classic', 'method':'GET',
                'url':f'dubbo://127.0.0.1:{args.provider_port}', 'description':'',
                'query':[], 'headers':[], 'body_kind':'none', 'body':'',
                'auth':{'kind':'none','token':'','username':'','password':''},
                'timeout_ms':10000, 'follow_redirects':False, 'verify_tls':True,
                'assertions':[], 'examples':[],
                'protocol':{'kind':'dubbo','source_version':1,'transport':'classic',
                    'serialization':'hessian2','service':'fixture.EchoService','method':'echo',
                    'version':'1.0.0','group':'',
                    'arguments':[{'id':'a','type_name':'java.lang.String',
                                  'value_source':'"private-dubbo-value"','secret':True}],
                    'attachments':[]}}
            data = {'schema_version':1,'collections':[{'id':'c','name':'API','description':'',
                    'requests':[source]}], 'environments':[],'active_environment_id':None}
            status, saved = call('POST', '/api/workspaces', {'id':'w','name':'Dubbo','data':data}, owner)
            assert status == 200, saved
            status, denied = call('POST', '/api/execute', {'workspace_id':'w','request':source}, stranger)
            assert status == 404, denied
            status, result = call('POST', '/api/execute', {'workspace_id':'w','request':source}, owner)
            assert status == 200 and result['status'] == 200, result
            assert json.loads(result['body']) == 'echo:private-dubbo-value', result
            status, history = call('GET', '/api/workspaces/w/history', token=owner)
            assert status == 200 and 'private-dubbo-value' not in json.dumps(history), history
            status, restored = call('GET', '/api/workspaces/w', token=owner)
            assert status == 200 and restored['data']['collections'][0]['requests'][0]['protocol']['arguments'][0]['value_source'] == '"private-dubbo-value"', restored
            null_request = json.loads(json.dumps(source))
            null_request['protocol']['method'] = 'maybeNull'
            null_request['protocol']['arguments'][0]['value_source'] = '"null"'
            status, result = call('POST', '/api/execute', {'workspace_id':'w','request':null_request}, owner)
            assert status == 200 and result['status'] == 200 and json.loads(result['body']) is None, result
            error_request = json.loads(json.dumps(source))
            error_request['protocol']['method'] = 'fail'
            error_request['protocol']['arguments'][0]['value_source'] = '"expected"'
            status, result = call('POST', '/api/execute', {'workspace_id':'w','request':error_request}, owner)
            assert status == 200 and result['status'] == 500 and 'fixture:expected' in result['body'], result
            evidence = {'format':'moleapi-dubbo-hosted-qa-v1',
                        'worker_sha256':hashlib.sha256(args.jar.read_bytes()).hexdigest(),
                        'cases':['owner_isolation','echo','history_privacy','source_preservation',
                                 'null','remote_exception']}
            args.output.write_text(json.dumps(evidence, indent=2) + '\n')
            print('PASS hosted classic Dubbo owner/privacy/null/exception', flush=True)
        finally:
            server.terminate()
            try: server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.kill(); server.wait(timeout=10)


if __name__ == '__main__':
    main()
