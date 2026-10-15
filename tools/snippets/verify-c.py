#!/usr/bin/env python3
"""Compile and send trusted original C/libcurl snippets to a local HTTP fixture."""
import argparse
from email import policy
from email.parser import BytesParser
import hashlib
import http.server
import json
from pathlib import Path
import queue
import subprocess
import threading
import urllib.parse

ROOT = Path(__file__).resolve().parents[2]
FILE_BYTES = b'\x00\xff\x80\nX'
PROFILES = ('core', 'form', 'form-empty', 'text', 'json-numbers', 'json-crlf',
            'multipart', 'multipart-text', 'multipart-meta')


def run(command, cwd=ROOT):
    result = subprocess.run(command, cwd=cwd, capture_output=True, text=True, timeout=120)
    if result.returncode:
        raise RuntimeError(f'{command}: {result.returncode}\n{result.stdout}\n{result.stderr}')
    return result.stdout


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--cc', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    assert args.cc.is_absolute() and args.cc.is_file()
    assert args.output.is_absolute()
    args.output.mkdir(exist_ok=False)
    records = queue.Queue()

    class Fixture(http.server.BaseHTTPRequestHandler):
        def do_POST(self):
            body = self.rfile.read(int(self.headers.get('Content-Length', '0')))
            records.put((self.path, dict(self.headers), body))
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b'{"ok":true}')

        def log_message(self, *_):
            pass

    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Fixture)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        for profile in PROFILES:
            folder = args.output / profile
            run(['cargo', 'run', '-q', '--locked', '-p', 'moleapi-generation',
                 '--example', 'fixture_snippets', '--', str(folder),
                 f'http://127.0.0.1:{server.server_port}', 'c', profile])
            source = (folder / 'fixture-request.json').read_bytes()
            manifest = json.loads((folder / 'fixture-manifest.json').read_text())
            code = (folder / 'snippet.c').read_bytes()
            assert manifest['target'] == 'c' and manifest['profile'] == profile
            assert digest(source) == manifest['source_sha256']
            assert digest(code) == manifest['files']['snippet.c']
            run([str(args.cc), '-std=c11', '-Wall', '-Wextra', '-Werror',
                 str(folder / 'snippet.c'), '-lcurl', '-o', str(folder / 'consumer')])
            (folder / 'upload.bin').write_bytes(FILE_BYTES)
            (folder / 'upload-鼹鼠.bin').write_bytes(FILE_BYTES)
            run([str(folder / 'consumer')], folder)
            path, headers, body = records.get(timeout=10)
            headers = {key.lower(): value for key, value in headers.items()}
            assert headers['authorization'] == 'Bearer fixture-token', profile
            assert headers['x-test'] == 'moleapi-fixture', profile
            assert urllib.parse.parse_qs(urllib.parse.urlsplit(path).query) == {
                'existing': ['one'], 'q': ['space & +']}, profile
            if profile == 'core':
                assert json.loads(body) == {'message': '鼹鼠 "quotes" & +'}
            elif profile == 'form':
                assert urllib.parse.parse_qs(body.decode(), keep_blank_values=True) == {
                    'label': ['鼹鼠 "quotes" & +', 'second'], 'empty': ['']}
            elif profile == 'form-empty':
                assert body == b'' and headers['content-type'].startswith('application/x-www-form-urlencoded')
            elif profile == 'text':
                assert body == '鼹鼠 "quotes" & +\r\nline\0\vend'.encode()
            elif profile == 'json-numbers':
                assert body == b'{"id":9007199254740993,"max":18446744073709551615,"decimal":1.0000000000000000000000001}'
            elif profile == 'json-crlf':
                assert body == '{\r\n  "message": "鼹鼠"\r\n}'.encode()
            else:
                message = BytesParser(policy=policy.default).parsebytes(
                    f'Content-Type: {headers["content-type"]}\r\nMIME-Version: 1.0\r\n\r\n'.encode() + body)
                parts = list(message.iter_parts())
                if profile == 'multipart-text':
                    assert [part.get_param('name', header='content-disposition') for part in parts] == [
                        'label', 'label', 'empty']
                    assert [part.get_payload(decode=True) for part in parts] == [
                        '鼹鼠 "quotes"\r\nvalue'.encode(), b'second', b'']
                elif profile == 'multipart-meta':
                    assert [part.get_param('name', header='content-disposition') for part in parts] == [
                        'metadata', 'upload']
                    assert parts[0].get_content_type() == 'text/x-moleapi-field'
                    assert parts[0].get_payload(decode=True) == '鼹鼠 "quotes"\r\nvalue'.encode()
                    assert parts[1].get_content_type() == 'application/x-moleapi-file'
                    assert parts[1].get_filename() == 'upload-鼹鼠.bin'
                    assert parts[1].get_payload(decode=True) == FILE_BYTES
                else:
                    assert [part.get_param('name', header='content-disposition') for part in parts] == [
                        'label', 'label', 'upload']
                    assert parts[2].get_payload(decode=True) == FILE_BYTES
            print(f'PASS C/libcurl {profile}', flush=True)
        assert records.empty()
        missing = args.output / 'multipart' / 'upload.bin'
        missing.unlink()
        result = subprocess.run([str(args.output / 'multipart' / 'consumer')],
                                cwd=missing.parent, capture_output=True, timeout=15)
        assert result.returncode != 0, 'missing file was reported as success'
        assert records.empty(), 'missing file sent a partial request'
        print('PASS C/libcurl missing multipart file fails before request', flush=True)
    finally:
        server.shutdown()
        server.server_close()


if __name__ == '__main__':
    main()
