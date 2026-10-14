#!/usr/bin/env python3
"""Run original Flow/Elm model codecs with actual compilers in explicit trusted QA."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess

spec = importlib.util.spec_from_file_location('native_models_qa', Path(__file__).with_name('verify-native.py'))
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)

FLOW = r'''// @flow
import type {Pet} from './models';
const Convert = require('./models');
module.exports = (source: string): void => {
const decoded: Pet = Convert.toPet(source);
const withDate = Object.hasOwn(JSON.parse(source), 'observed');
if (withDate) {
  const date = decoded.observed;
  if (!(date instanceof Date) || date.toISOString() !== '2026-10-15T00:00:00.000Z') throw Error('Native Date lost');
}
if (decoded.id !== 0 || decoded.status !== 'ready' || decoded.note !== null) throw Error('Native fields lost');
const child = decoded.friend;
if (!child || child.id !== 2 || child.status !== 'done' || child.note !== '鼹鼠 "quotes" & +') throw Error('Recursive native fields lost');
console.log(JSON.stringify(JSON.parse(Convert.petToJson(decoded))));
const built: Pet = {id: 0, status: 'ready', note: null, friend: {id: 2, status: 'done', note: '鼹鼠 "quotes" & +'}, ...(withDate ? {observed: new Date('2026-10-15T00:00:00.000Z')} : {})};
console.log(JSON.stringify(JSON.parse(Convert.petToJson(built))));
let rejected = false;
try {Convert.toPet(source.replace('"ready"', '"invalid"'));} catch (expected) {rejected = true;}
if (!rejected) throw Error('Unknown enum accepted');
if (withDate) {
  const invalid = JSON.parse(source); invalid.observed = 'invalid-date';
  let dateRejected = false;
  try {Convert.toPet(JSON.stringify(invalid));} catch (expected) {dateRejected = true;}
  if (!dateRejected) throw Error('Invalid date accepted');
}
};
'''

BABEL = r'''import {pathToFileURL} from 'node:url';
import {mkdir, writeFile} from 'node:fs/promises';
const root = process.argv[2];
const babel = await import(pathToFileURL(root + '/node_modules/@babel/core/lib/index.js'));
const plugin = await import(pathToFileURL(root + '/node_modules/@babel/plugin-transform-flow-strip-types/lib/index.js'));
await mkdir('compiled', {recursive: true});
for (const source of ['models.js', 'RuntimeFixture.js']) {
  const result = await babel.transformFileAsync(source, {plugins: [plugin.default], configFile: false, babelrc: false});
  await writeFile('compiled/' + source, result.code);
}
'''

ELM = r'''port module Main exposing (main)
import Json.Decode as Decode
import Json.Encode as Encode
import Platform
import QuickType exposing (Pet(..), Status(..), pet, petToString)
port result : Encode.Value -> Cmd msg
port start : (String -> msg) -> Sub msg
type Msg = Run String
main : Program () () Msg
main = Platform.worker { init = \_ -> ( (), Cmd.none ), update = \(Run source) _ -> run source, subscriptions = \_ -> start Run }
run : String -> ( (), Cmd Msg )
run source =
    case Decode.decodeString pet source of
        Err error -> ( (), result (Encode.object [ ("error", Encode.string (Decode.errorToString error)) ]) )
        Ok ((Pet fields) as decoded) ->
            let
                childOk = case fields.friend of
                    Just (Pet child) -> child.id == 2 && child.status == Done && child.note == Just "鼹鼠 \"quotes\" & +"
                    Nothing -> False
                built = Pet { id = 0, status = Ready, note = Nothing, friend = Just (Pet { id = 2, status = Done, note = Just "鼹鼠 \"quotes\" & +", friend = Nothing }) }
                rejected = case Decode.decodeString pet (String.replace "\"ready\"" "\"invalid\"" source) of
                    Err _ -> True
                    Ok _ -> False
            in
            if fields.id == 0 && fields.status == Ready && fields.note == Nothing && childOk && rejected then
                ( (), result (Encode.list Encode.string [ petToString decoded, petToString built ]) )
            else
                ( (), result (Encode.object [ ("error", Encode.string "Native model fields or enum rejection lost") ]) )
'''

ELM_RUN = r'''const fs = require('node:fs');
const {Elm} = require('./compiled.js');
const app = Elm.Main.init({flags: null});
const timeout = setTimeout(() => {throw Error('Elm port completion timeout');}, 10000);
app.ports.result.subscribe(value => {
  clearTimeout(timeout);
  if (!Array.isArray(value) || value.length !== 2) throw Error(JSON.stringify(value));
  for (const json of value) console.log(JSON.stringify(JSON.parse(json)));
});
app.ports.start.send(fs.readFileSync('sample.json', 'utf8'));
'''

ELM_FLAT = r'''port module Main exposing (main)
import Json.Decode as Decode
import Json.Encode as Encode
import Platform
import QuickType exposing (Pet, Status(..), pet, petToString)
port result : Encode.Value -> Cmd msg
port start : (String -> msg) -> Sub msg
type Msg = Run String
main : Program () () Msg
main = Platform.worker { init = \_ -> ( (), Cmd.none ), update = \(Run source) _ -> run source, subscriptions = \_ -> start Run }
run : String -> ( (), Cmd Msg )
run source =
    case Decode.decodeString pet source of
        Err error -> ( (), result (Encode.object [ ("error", Encode.string (Decode.errorToString error)) ]) )
        Ok decoded ->
            let
                built : Pet
                built = { id = 0, status = Ready, note = Nothing }
                rejected = case Decode.decodeString pet (String.replace "\"ready\"" "\"invalid\"" source) of
                    Err _ -> True
                    Ok _ -> False
            in
            if decoded.id == 0 && decoded.status == Ready && decoded.note == Nothing && rejected then
                ( (), result (Encode.list Encode.string [ petToString decoded, petToString built ]) )
            else
                ( (), result (Encode.object [ ("error", Encode.string "Native alias fields lost") ]) )
'''


def verify(args):
    assert args.fixtures.is_absolute() and args.fixtures.is_dir()
    assert args.node.is_absolute() and args.node.is_file()
    root = args.fixtures / args.target
    manifest = json.loads((root / 'moleapi-generation.json').read_text())
    assert manifest['engine'] == 'quicktype-core@26.0.0+moleapi.1' and manifest['target'] == args.target
    source = (root / 'fixture-source.json').read_bytes()
    assert json.loads(source)['openapi'] == args.dialect
    assert hashlib.sha256(source).hexdigest() == manifest['source_sha256']
    before = {file['path']: hashlib.sha256((root / file['path']).read_bytes()).hexdigest() for file in manifest['files']}
    assert all(before[file['path']] == file['sha256'] for file in manifest['files'])
    consumer = args.fixtures / (args.target + '-consumer')
    consumer.mkdir(exist_ok=False)
    with_date = 'observed' in json.loads(source)['components']['schemas']['Pet']['properties']
    flat = 'friend' not in json.loads(source)['components']['schemas']['Pet']['properties']
    sample = dict(native.SAMPLE)
    if flat:
        sample.pop('friend')
    if with_date:
        sample['observed'] = '2026-10-15T00:00:00.000Z'
    (consumer / 'sample.json').write_text(json.dumps(sample, ensure_ascii=False) + '\n')
    dependencies = {}
    tools = {'node': native.run([str(args.node), '--version'], consumer).strip()}
    if args.target == 'model-flow':
        assert args.flow and args.flow.is_absolute() and args.flow.is_file()
        assert args.tools and args.tools.is_absolute() and (args.tools / 'package-lock.json').is_file()
        shutil.copyfile(root / 'models.js', consumer / 'models.js')
        (consumer / 'RuntimeFixture.js').write_text(FLOW)
        (consumer / '.flowconfig').write_text('[ignore]\n[include]\n[libs]\n[options]\nall=true\n')
        native.run([str(args.flow), 'full-check', '--no-cgroup', '--max-workers', '2'], consumer)
        negative = consumer / 'NegativeFixture.js'
        negative.write_text("// @flow\nimport type {Pet} from './models';\nconst invalid: Pet = {id: 'wrong', status: 'invalid'};\n")
        try:
            result = subprocess.run([str(args.flow), 'full-check', '--no-cgroup', '--max-workers', '2', '--json'], cwd=consumer, text=True, capture_output=True, timeout=120)
            assert result.returncode == 2, result.stdout + result.stderr
            errors = json.loads(result.stdout)['errors']
            assert len(errors) == 2, errors
            text = json.dumps(errors)
            assert 'id' in text and 'status' in text
            (consumer / 'type-errors.json').write_text(result.stdout)
        finally:
            negative.unlink()
        (consumer / 'compile.mjs').write_text(BABEL)
        native.run([str(args.node), 'compile.mjs', str(args.tools)], consumer)
        (consumer / 'RuntimeRunner.cjs').write_text("require('./compiled/RuntimeFixture.js')(require('node:fs').readFileSync('sample.json', 'utf8'));\n")
        output = native.run([str(args.node), 'RuntimeRunner.cjs'], consumer)
        tools['flow'] = native.run([str(args.flow), 'version'], consumer).strip()
        dependencies['npm_lock_sha256'] = hashlib.sha256((args.tools / 'package-lock.json').read_bytes()).hexdigest()
    else:
        assert args.target == 'model-elm'
        assert args.elm and args.elm.is_absolute() and args.elm.is_file()
        assert args.elm_home and args.elm_home.is_absolute()
        env = dict(os.environ, ELM_HOME=str(args.elm_home))
        (consumer / 'src').mkdir()
        shutil.copyfile(root / 'models.elm', consumer / 'src/QuickType.elm')
        (consumer / 'src/Main.elm').write_text(ELM_FLAT if flat else ELM)
        version = native.run([str(args.elm), '--version'], consumer).strip()
        config = {'type': 'application', 'source-directories': ['src'], 'elm-version': version,
                  'dependencies': {'direct': {'elm/core': '1.0.5', 'elm/json': '1.1.3', 'NoRedInk/elm-json-decode-pipeline': '1.0.0'}, 'indirect': {}},
                  'test-dependencies': {'direct': {}, 'indirect': {}}}
        (consumer / 'elm.json').write_text(json.dumps(config, indent=2) + '\n')
        native.run([str(args.elm), 'make', 'src/Main.elm', '--optimize', '--output=compiled.js'], consumer, env)
        (consumer / 'RuntimeFixture.cjs').write_text(ELM_RUN)
        output = native.run([str(args.node), 'RuntimeFixture.cjs'], consumer)
        tools['elm'] = version
        dependencies = config['dependencies']['direct']
    if flat:
        values = [json.loads(line) for line in output.splitlines() if line.startswith('{')]
        assert len(values) == 2 and all(value == sample for value in values)
    else:
        native.check(output)
    if with_date:
        assert all(json.loads(line)['observed'] == sample['observed'] for line in output.splitlines() if line.startswith('{'))
    assert all(hashlib.sha256((root / path).read_bytes()).hexdigest() == digest for path, digest in before.items())
    evidence = {'target': args.target, 'dialect': args.dialect, 'engine': manifest['engine'], 'platform': platform.platform(),
                'tools': tools, 'dependencies': dependencies, 'source_sha256': manifest['source_sha256'],
                'generated_source_sha256': before, 'profiles': ['decoded', 'constructed', 'recursive', 'enum', 'zero', 'null', 'unicode', 'invalid-enum'], 'result': 'passed'}
    if flat:
        evidence['profiles'] = ['decoded', 'constructed', 'nonrecursive-alias', 'enum', 'zero', 'null', 'invalid-enum']
    if with_date:
        evidence['profiles'] += ['native-date', 'invalid-date']
    (consumer / 'runtime-evidence.json').write_text(json.dumps(evidence, indent=2) + '\n')
    print(f'PASS {args.target}/{args.dialect}: actual compiler and original native models')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixtures', required=True, type=Path)
    parser.add_argument('--dialect', required=True, choices=['3.0.3', '3.1.0'])
    parser.add_argument('--target', required=True, choices=['model-flow', 'model-elm'])
    parser.add_argument('--node', required=True, type=Path)
    for name in ['flow', 'tools', 'elm', 'elm-home']:
        parser.add_argument('--' + name, type=Path)
    verify(parser.parse_args())
