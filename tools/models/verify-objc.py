#!/usr/bin/env python3
"""Compile original Quicktype models with actual Apple Foundation and ARC."""
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

OBJC = r'''#import "models.h"
static void check(BOOL condition, NSString *message) {
    if (!condition) @throw [NSException exceptionWithName:@"ModelQA" reason:message userInfo:nil];
}
int main(void) {
    @autoreleasepool {
        NSError *error = nil;
        NSData *source = [NSData dataWithContentsOfFile:@"sample.json"];
        QTPet *decoded = [QTPet fromData:source error:&error];
        check(decoded != nil && error == nil, @"Native decode failed");
        check(decoded.identifier == 0 && (decoded.note == nil || (id)decoded.note == NSNull.null), @"Zero/null lost");
        check(decoded.status == QTStatus.ready, @"Native enum lost");
        check([decoded.friend isKindOfClass:QTPet.class] && decoded.friend.identifier == 2, @"Native recursive model lost");
        check(decoded.friend.status == QTStatus.done, @"Nested enum lost");
        check([decoded.friend.note isEqualToString:@"鼹鼠 \"quotes\" & +"], @"Unicode lost");
        NSString *output = [decoded toJSON:NSUTF8StringEncoding error:&error];
        check(output != nil && error == nil, @"Native encoding failed");
        puts(output.UTF8String);
        QTPet *child = [QTPet new];
        child.identifier = 2;
        child.status = QTStatus.done;
        child.note = @"鼹鼠 \"quotes\" & +";
        QTPet *built = [QTPet new];
        built.identifier = 0;
        built.status = QTStatus.ready;
        built.friend = child;
        output = [built toJSON:NSUTF8StringEncoding error:&error];
        check(output != nil && error == nil, @"Native construction encoding failed");
        puts(output.UTF8String);
        error = nil;
        QTPet *invalid = [QTPet fromJSON:@"{\"id\":0,\"status\":\"invalid\"}" encoding:NSUTF8StringEncoding error:&error];
        check(invalid == nil && error != nil, @"Unknown enum silently accepted");
        error = nil;
        invalid = [QTPet fromJSON:@"\"not-an-object\"" encoding:NSUTF8StringEncoding error:&error];
        check(invalid == nil && error != nil, @"Non-object silently accepted");
        check([QTPet fromData:source error:NULL] != nil, @"NULL error output failed");
        check([built toData:NULL] != nil, @"NULL encoding error output failed");
    }
    return 0;
}
'''


def verify(args):
    if platform.system() != 'Darwin':
        raise RuntimeError('Actual Apple Foundation/Xcode runtime required')
    assert args.fixtures.is_absolute() and args.fixtures.is_dir()
    assert args.xcrun.is_absolute() and args.xcrun.is_file()
    root = args.fixtures / 'model-objc'
    manifest = json.loads((root / 'moleapi-generation.json').read_text())
    assert manifest['target'] == 'model-objc'
    assert manifest['engine'] in ('quicktype-core@26.0.0', 'quicktype-core@26.0.0+moleapi.1')
    source = (root / 'fixture-source.json').read_bytes()
    assert json.loads(source)['openapi'] == args.dialect
    assert hashlib.sha256(source).hexdigest() == manifest['source_sha256']
    before = {file['path']: hashlib.sha256((root / file['path']).read_bytes()).hexdigest() for file in manifest['files']}
    assert all(before[file['path']] == file['sha256'] for file in manifest['files'])
    consumer = args.fixtures / 'model-objc-consumer'
    consumer.mkdir(exist_ok=False)
    for name in ('models.h', 'models.m'):
        shutil.copyfile(root / name, consumer / name)
    (consumer / 'Main.m').write_text(OBJC)
    (consumer / 'sample.json').write_text(json.dumps(native.SAMPLE, ensure_ascii=False) + '\n')
    native.run([str(args.xcrun), 'clang', '-O2', '-fobjc-arc', '-fblocks', '-framework', 'Foundation',
                'models.m', 'Main.m', '-o', 'model-qa'], consumer)
    output = native.run([str(consumer / 'model-qa')], consumer)
    native.check(output)
    assert all(hashlib.sha256((root / path).read_bytes()).hexdigest() == digest for path, digest in before.items())
    evidence = {'target': 'model-objc', 'dialect': args.dialect, 'engine': manifest['engine'],
                'platform': platform.platform(), 'machine': platform.machine(),
                'compiler': native.run([str(args.xcrun), 'clang', '--version'], consumer).strip(),
                'source_sha256': manifest['source_sha256'], 'generated_source_sha256': before,
                'profiles': ['decoded', 'constructed', 'recursive', 'zero', 'enum', 'null', 'unicode',
                             'invalid-enum', 'invalid-object', 'null-error-output'], 'result': 'passed'}
    (consumer / 'runtime-evidence.json').write_text(json.dumps(evidence, indent=2) + '\n')
    print(f'PASS Objective-C/{args.dialect}: original native Apple models and errors')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixtures', required=True, type=Path)
    parser.add_argument('--dialect', required=True, choices=['3.0.3', '3.1.0'])
    parser.add_argument('--xcrun', required=True, type=Path)
    verify(parser.parse_args())
