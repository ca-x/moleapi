#!/usr/bin/env python3
"""Compile/run original Crystal model/JSON::Serializable output in explicit trusted QA."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import platform
import shutil

spec = importlib.util.spec_from_file_location('native_models_qa', Path(__file__).with_name('verify-native.py'))
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)

CRYSTAL = r'''require "./models"
def check_native(value : Bool)
  raise "Native model fields lost" unless value
end
source = File.read("sample.json")
wire_values = JSON.parse(File.read("enum-values.json")).as_a.map(&.as_s)
ready = Status.from_json(wire_values[0].to_json)
done = Status.from_json(wire_values[1].to_json)
wire_values.each do |value|
  check_native(JSON.parse(Status.from_json(value.to_json).to_json).as_s == value)
end
decoded = Pet.from_json(source)
check_native(decoded.id == 0 && decoded.status == ready && decoded.note.nil?)
child = decoded.friend.not_nil!
check_native(child.id == 2 && child.status == done && child.note == "鼹鼠 \"quotes\" & +")
puts decoded.to_json
child = Pet.new(id: 2, status: done, note: "鼹鼠 \"quotes\" & +")
built = Pet.new(id: 0, status: ready, note: nil, friend: child)
puts built.to_json
rejected = false
begin
  invalid = JSON.parse(source)
  invalid.as_h["status"] = JSON::Any.new("invalid")
  Pet.from_json(invalid.to_json)
rescue
  rejected = true
end
check_native(rejected)
'''


def verify(args):
    assert args.fixtures.is_absolute() and args.fixtures.is_dir()
    assert args.crystal.is_absolute() and args.crystal.is_file()
    root = args.fixtures / 'model-crystal'
    manifest = json.loads((root / 'moleapi-generation.json').read_text())
    assert manifest['target'] == 'model-crystal' and manifest['engine'] == 'quicktype-core@26.0.0+moleapi.1'
    source = (root / 'fixture-source.json').read_bytes()
    assert json.loads(source)['openapi'] == args.dialect
    assert hashlib.sha256(source).hexdigest() == manifest['source_sha256']
    before = {file['path']: hashlib.sha256((root / file['path']).read_bytes()).hexdigest() for file in manifest['files']}
    assert all(before[file['path']] == file['sha256'] for file in manifest['files'])
    consumer = args.fixtures / 'model-crystal-consumer'
    consumer.mkdir(exist_ok=False)
    shutil.copyfile(root / 'models.cr', consumer / 'models.cr')
    (consumer / 'RuntimeFixture.cr').write_text(CRYSTAL)
    wire_values = json.loads(source)['components']['schemas']['Pet']['properties']['status']['enum']
    sample = json.loads(json.dumps(native.SAMPLE))
    sample['status'] = wire_values[0]
    sample['friend']['status'] = wire_values[1]
    (consumer / 'sample.json').write_text(json.dumps(sample, ensure_ascii=False) + '\n')
    (consumer / 'enum-values.json').write_text(json.dumps(wire_values, ensure_ascii=False) + '\n')
    native.run([str(args.crystal), 'build', '--release', 'RuntimeFixture.cr', '-o', 'model-qa'], consumer)
    output = native.run([str(consumer / 'model-qa')], consumer)
    records = [json.loads(line) for line in output.splitlines() if line.startswith('{')]
    assert len(records) == 2
    for value in records:
        assert value['id'] == 0 and 'note' not in value
        assert value['status'] == wire_values[0] and value['friend']['status'] == wire_values[1]
        assert value['friend']['id'] == 2 and value['friend']['note'] == native.NOTE
    assert all(hashlib.sha256((root / path).read_bytes()).hexdigest() == digest for path, digest in before.items())
    evidence = {'target': 'model-crystal', 'dialect': args.dialect, 'engine': manifest['engine'], 'platform': platform.platform(),
                'crystal': native.run([str(args.crystal), '--version'], consumer).strip(), 'source_sha256': manifest['source_sha256'],
                'generated_source_sha256': before, 'nullable_serialization': 'omitted', 'integer_range': 'Int32', 'enum_values': wire_values,
                'profiles': ['decoded', 'constructed', 'recursive', 'zero', 'enum', 'nullable-value', 'unicode', 'invalid-enum'], 'result': 'passed'}
    (consumer / 'runtime-evidence.json').write_text(json.dumps(evidence, indent=2) + '\n')
    print(f'PASS Crystal/{args.dialect}: original native model compiler/runtime')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixtures', required=True, type=Path)
    parser.add_argument('--dialect', required=True, choices=['3.0.3', '3.1.0'])
    parser.add_argument('--crystal', required=True, type=Path)
    verify(parser.parse_args())
