#!/usr/bin/env python3
"""Run original generated Pike models with an explicitly selected trusted interpreter."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import platform
import shutil
import subprocess

spec = importlib.util.spec_from_file_location('native_models_qa', Path(__file__).with_name('verify-native.py'))
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)

PIKE = r'''#charset utf-8
import .models;
void ensure(int value, string message) { if (!value) error("%s\n", message); }
int main() {
    string source = utf8_to_string(Stdio.read_file("sample.json"));
    Pet decoded = Pet_from_JSON(Standards.JSON.decode(source));
    ensure(decoded->id == 0 && decoded->status == READY, "Native root fields lost");
    ensure(decoded->note == Standards.JSON.null, "Explicit JSON null lost");
    ensure(objectp(decoded->friend) && decoded->friend->id == 2 && decoded->friend->status == DONE,
           "Recursive native Pet object lost");
    ensure(decoded->friend->note == "鼹鼠 \"quotes\" & +", "Unicode or quoting lost");
    write("%s\n", string_to_utf8(decoded->encode_json()));
    Pet child = Pet(); child->id = 2; child->status = DONE; child->note = "鼹鼠 \"quotes\" & +";
    Pet built = Pet(); built->id = 0; built->status = READY; built->note = Standards.JSON.null; built->friend = child;
    write("%s\n", string_to_utf8(built->encode_json()));
    mixed rejected = catch {Pet_from_JSON(Standards.JSON.decode(replace(source, "\"ready\"", "\"invalid\"")));};
    ensure(!!rejected, "Unknown enum accepted");
    __CONTAINERS__
    return 0;
}
'''

CONTAINERS = r'''mapping wire = Standards.JSON.decode(utf8_to_string(Stdio.read_file("sample.json")));
    Envelope envelope = Envelope_from_JSON((["primary":wire,"pets":({wire,wire}),"lookup":(["蓝 +":wire,"007":wire]),"shades":({"ready","done"}),"counts":(["蓝 +":0,"007":7])]));
    ensure(objectp(envelope->primary) && envelope->primary->id == 0, "Direct native model lost");
    ensure(sizeof(envelope->pets)==2 && objectp(envelope->pets[0]) && objectp(envelope->pets[1]), "Native model array lost");
    ensure(sizeof(envelope->lookup)==2 && objectp(envelope->lookup["蓝 +"]) && objectp(envelope->lookup["007"]), "Native map/key lost");
    ensure(equal(envelope->shades, ({READY,DONE})), "Enum array lost");
    ensure(envelope->counts["蓝 +"] == 0 && envelope->counts["007"] == 7, "Zero/count map lost");
    PetArray pets = PetArray_from_JSON(({wire,wire})); PetMap lookup = PetMap_from_JSON((["蓝 +":wire]));
    ensure(sizeof(pets)==2 && objectp(pets[0]) && objectp(lookup["蓝 +"]), "Top-level model containers lost");
    ensure(!!catch {Envelope_from_JSON((["primary":wire,"pets":({wire}),"lookup":([]),"shades":({"invalid"})]));}, "Container enum rejection lost");
    Stdio.write_file("container-evidence.json", string_to_utf8(envelope->encode_json()));'''


def verify(args):
    assert args.fixtures.is_absolute() and args.fixtures.is_dir()
    assert args.pike.is_absolute() and args.pike.is_file()
    root = args.fixtures / 'model-pike'
    manifest = json.loads((root / 'moleapi-generation.json').read_text())
    assert manifest['target'] == 'model-pike' and manifest['engine'] == 'quicktype-core@26.0.0+moleapi.1'
    source = (root / 'fixture-source.json').read_bytes()
    assert json.loads(source)['openapi'] == args.dialect
    assert hashlib.sha256(source).hexdigest() == manifest['source_sha256']
    before = {file['path']: hashlib.sha256((root / file['path']).read_bytes()).hexdigest() for file in manifest['files']}
    assert all(before[file['path']] == file['sha256'] for file in manifest['files'])
    consumer = args.fixtures / 'model-pike-consumer'
    consumer.mkdir(exist_ok=False)
    shutil.copyfile(root / 'models.pmod', consumer / 'models.pmod')
    containers = 'Envelope' in json.loads(source)['components']['schemas']
    (consumer / 'RuntimeFixture.pike').write_text(PIKE.replace('__CONTAINERS__', CONTAINERS if containers else ''))
    (consumer / 'sample.json').write_text(json.dumps(native.SAMPLE, ensure_ascii=False) + '\n')
    output = native.run([str(args.pike), 'RuntimeFixture.pike'], consumer)
    native.check(output)
    if containers:
        values = json.loads((consumer / 'container-evidence.json').read_text())
        assert values['primary']['friend']['note'] == native.NOTE
        assert len(values['pets']) == 2 and values['lookup']['蓝 +']['id'] == 0
        assert values['shades'] == ['ready', 'done'] and values['counts'] == {'蓝 +': 0, '007': 7}
    assert all(hashlib.sha256((root / path).read_bytes()).hexdigest() == digest for path, digest in before.items())
    evidence = {'target': 'model-pike', 'dialect': args.dialect, 'engine': manifest['engine'],
                'platform': platform.platform(), 'pike': subprocess.check_output([str(args.pike), '--version'], cwd=consumer, stderr=subprocess.STDOUT, text=True, timeout=30).strip(),
                'source_sha256': manifest['source_sha256'], 'generated_source_sha256': before,
                'profiles': ['decoded', 'constructed', 'recursive-native-model', 'zero', 'enum', 'null', 'unicode', 'invalid-enum'], 'result': 'passed'}
    if containers:
        evidence['profiles'] += ['direct-model', 'model-array', 'model-map', 'top-level-containers', 'enum-array', 'integer-map', 'container-enum-rejection']
    (consumer / 'runtime-evidence.json').write_text(json.dumps(evidence, indent=2) + '\n')
    print(f'PASS Pike/{args.dialect}: original native model conversion')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixtures', required=True, type=Path)
    parser.add_argument('--dialect', required=True, choices=['3.0.3', '3.1.0'])
    parser.add_argument('--pike', required=True, type=Path)
    verify(parser.parse_args())
