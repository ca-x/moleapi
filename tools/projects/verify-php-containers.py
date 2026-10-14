#!/usr/bin/env python3
"""Call original default PHP nested model/map output through real authenticated HTTP."""
import argparse
import hashlib
import http.server
import importlib.util
import json
import os
from pathlib import Path
import platform
import subprocess
import threading
import urllib.parse

module = importlib.util.spec_from_file_location('moleapi_runtime_qa', Path(__file__).with_name('verify-runtime.py'))
runtime = importlib.util.module_from_spec(module)
module.loader.exec_module(runtime)

ENVELOPE = {'primary': {'id': 0, 'status': 'ready', 'note': runtime.NOTE},
            'pets': [{'id': 0, 'status': 'ready', 'note': runtime.NOTE}, {'id': 7, 'status': 'done'}],
            'lookup': {'蓝 +': {'id': 0, 'status': 'ready', 'note': runtime.NOTE},
                       '007': {'id': 7, 'status': 'done'}},
            'counts': {'蓝 +': 0, '007': 7}, 'note': None}

PHP = r'''
use OpenAPI\Client\Model\Envelope;
use OpenAPI\Client\ObjectSerializer;
function checkEnvelope(Envelope $value, bool $labelPresent = false): void {
    ensureFixture($value->getPrimary() instanceof Pet && $value->getPrimary()->getId() === 0
                  && $value->getPrimary()->getNote() === '鼹鼠 "quotes" & +', 'Nested model lost');
    $pets = $value->getPets(); $lookup = $value->getLookup(); $counts = $value->getCounts();
    ensureFixture(count($pets) === 2 && $pets[0] instanceof Pet && $pets[1] instanceof Pet
                  && $pets[1]->getStatus() === Pet::STATUS_DONE, 'Typed model array lost');
    ensureFixture(count($lookup) === 2 && $lookup['蓝 +'] instanceof Pet
                  && $lookup['007'] instanceof Pet && $lookup['007']->getId() === 7, 'Typed model map or string keys lost');
    ensureFixture($counts === ['蓝 +' => 0, '007' => 7], 'Integer map zero/key lost');
    $json = json_decode(json_encode(ObjectSerializer::sanitizeForSerialization($value), JSON_THROW_ON_ERROR), true, 512, JSON_THROW_ON_ERROR);
    ensureFixture(array_key_exists('note', $json) && $json['note'] === null, 'Required nullable presence lost');
    ensureFixture(array_key_exists('label', $json) === $labelPresent, 'Optional nullable presence lost');
    if ($labelPresent) ensureFixture($json['label'] === null, 'Explicit null label lost');
    ensureFixture($value->valid(), 'Required nullable note presence lost');
}
$envelope = $api->getEnvelope();
ensureFixture($envelope instanceof Envelope, 'Native response class lost');
checkEnvelope($envelope);
$primary = new Pet(['id' => 0, 'status' => Pet::STATUS_READY, 'note' => '鼹鼠 "quotes" & +']);
$secondary = new Pet(['id' => 7, 'status' => Pet::STATUS_DONE]);
$constructed = new Envelope(['primary' => $primary, 'pets' => [$primary, $secondary],
    'lookup' => ['蓝 +' => $primary, '007' => $secondary], 'counts' => ['蓝 +' => 0, '007' => 7], 'note' => null]);
checkEnvelope($constructed);
checkEnvelope($api->echoEnvelope(envelope: $constructed));
checkEnvelope($api->echoEnvelope(envelope: $envelope));
$constructed->setLabel(null);
checkEnvelope($api->echoEnvelope(envelope: $constructed), true);
$missing = json_decode(json_encode(ObjectSerializer::sanitizeForSerialization($constructed), JSON_THROW_ON_ERROR));
unset($missing->note);
$invalid = ObjectSerializer::deserialize($missing, Envelope::class);
ensureFixture(!$invalid->valid() && !$invalid->isNullableSetToNull('note'), 'Missing required nullable property became valid null');
'''


class Fixture(runtime.Fixture):
    def request(self):
        if urllib.parse.urlsplit(self.path).path != '/envelopes':
            return super().request()
        self.observations.append((self.command, '/envelopes'))
        try:
            assert self.headers.get('Authorization') == f'Bearer {runtime.TOKEN}'
            if self.command == 'POST':
                assert self.headers.get_content_type() == 'application/json'
                body = runtime.read_chunked_body(self.rfile) if self.headers.get('Transfer-Encoding', '').lower() == 'chunked' else self.rfile.read(int(self.headers.get('Content-Length', '0')))
                decoded = json.loads(body)
                expected = dict(ENVELOPE)
                if self.observations.count(('POST', '/envelopes')) == 3:
                    expected['label'] = None
                assert decoded == expected, body
                self.respond(200, decoded)
                return
            else:
                assert self.command == 'GET'
            self.respond(200, ENVELOPE)
        except Exception as error:
            self.errors.append(str(error))
            self.respond(500, {'message': str(error)})

    do_GET = request
    do_POST = request


def verify(args):
    assert args.root.is_absolute() and args.root.is_dir()
    assert args.php.is_absolute() and args.php.is_file()
    assert args.composer.is_absolute() and args.composer.is_file()
    records = []
    for version in ('3.0.3', '3.1.0'):
        root = args.root / version / 'php'
        manifest = json.loads((root / 'moleapi-generation.json').read_text())
        assert manifest['target'] == 'php'
        before = {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest() for p in (root / 'lib').rglob('*.php')}
        assert 'lib/Model/Envelope.php' in before
        runtime.run([str(args.php), str(args.composer), 'install', '--no-dev', '--no-interaction', '--no-plugins', '--no-scripts', '--prefer-dist'], root, timeout=600)
        dependencies = subprocess.check_output([str(args.php), str(args.composer), 'show', '--format=json', '--no-plugins'], cwd=root, text=True)
        (root / 'runtime-consumer-dependencies.json').write_text(dependencies)
        (root / 'runtime_containers.php').write_text(runtime.PHP_CORE.replace('__PHP_EXTENDED__', PHP))
        Fixture.observations.clear()
        Fixture.errors.clear()
        server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Fixture)
        thread = threading.Thread(target=server.serve_forever)
        thread.start()
        try:
            runtime.run([str(args.php), 'runtime_containers.php'], root, dict(os.environ, FIXTURE_URL=f'http://127.0.0.1:{server.server_port}'))
            assert not Fixture.errors, Fixture.errors
            assert Fixture.observations == [('GET', '/pets/7'), ('POST', '/pets/7'), ('GET', '/fail'), ('GET', '/envelopes')] + [('POST', '/envelopes')] * 3, Fixture.observations
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)
            assert not thread.is_alive()
        assert all(hashlib.sha256((root / path).read_bytes()).hexdigest() == digest for path, digest in before.items())
        record = {'target': 'php', 'dialect': version, 'profile': 'containers', 'engine': manifest['engine'],
                  'platform': platform.platform(), 'requests': Fixture.observations.copy(),
                  'php': subprocess.check_output([str(args.php), '--version'], text=True).strip(),
                  'source_sha256': hashlib.sha256((root / 'fixture-source.json').read_bytes()).hexdigest(),
                  'generated_source_sha256': before, 'lock_sha256': hashlib.sha256((root / 'composer.lock').read_bytes()).hexdigest(), 'result': 'passed'}
        records.append(record)
        (args.root / 'runtime-evidence.json').write_text(json.dumps(records, indent=2) + '\n')
        print(f'PASS PHP nested maps/{version}', flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    parser.add_argument('--php', type=Path, required=True)
    parser.add_argument('--composer', type=Path, required=True)
    verify(parser.parse_args())
