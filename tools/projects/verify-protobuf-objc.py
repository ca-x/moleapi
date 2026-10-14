#!/usr/bin/env python3
"""Build original Objective-C gRPC clients on Apple and call a real Python server."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import platform
import shutil
import subprocess
import sys


def run(command, cwd, timeout=1800):
    print('RUN', ' '.join(map(str, command)), flush=True)
    subprocess.run(command, cwd=cwd, check=True, timeout=timeout)


def verify(args):
    if platform.system() != 'Darwin':
        raise RuntimeError('Actual Apple Foundation/Xcode runtime required')
    import grpc
    root = args.root
    assert root.is_absolute() and root.is_dir()
    assert args.server_dir.is_absolute() and args.server_dir.is_dir()
    descriptor = hashlib.sha256((root / 'descriptor-set.bin').read_bytes()).hexdigest()
    assert descriptor == hashlib.sha256((args.server_dir / 'descriptor-set.bin').read_bytes()).hexdigest(), 'Server/client schema differs'
    manifest = json.loads((root / 'moleapi-generation.json').read_text())
    assert manifest['target'] == 'protobuf-objc' and 'grpc-plugin=true' in manifest['engine']
    before = {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
              for p in root.rglob('*') if p.suffix in ('.h', '.m') and 'runtime-consumer' not in p.relative_to(root).parts}
    assert 'Fixture.pbrpc.m' in before and 'models/Types.pbobjc.m' in before
    consumer = root / 'runtime-consumer'
    consumer.mkdir(exist_ok=False)
    scripts = Path(__file__).resolve().parent / 'objc'
    shutil.copyfile(scripts / 'RuntimeFixture.m', consumer / 'RuntimeFixture.m')
    (consumer / 'Podfile').write_text("""source 'https://cdn.cocoapods.org/'
platform :osx, '12.0'
project 'FixtureQA.xcodeproj'
target 'FixtureQA' do
  pod 'gRPC-ProtoRPC', '1.84.0'
  pod 'Protobuf', '5.36.1'
end
""")
    run(['ruby', str(scripts / 'create-project.rb'), str(root)], consumer)
    run(['pod', 'install'], consumer)
    run(['xcodebuild', '-workspace', 'FixtureQA.xcworkspace', '-scheme', 'FixtureQA',
         '-configuration', 'Release', '-derivedDataPath', str(consumer / 'build'),
         'CODE_SIGNING_ALLOWED=NO', 'build'], consumer)
    binary = consumer / 'build/Build/Products/Release/FixtureQA'
    assert binary.is_file()
    sys.path.insert(0, str(args.server_dir))
    from models import types_pb2, common_pb2
    import fixture_pb2_grpc
    expected = types_pb2.Echo(text='鼹鼠 "quotes" & +', payload=b'\x00\xff\x80A',
                             shade=common_pb2.READY, labels=['red', '蓝'], note='detail')
    expected.observed.seconds = 1234567
    expected.observed.nanos = 123
    other = types_pb2.Echo()
    other.CopyFrom(expected)
    other.code = 0
    observations = []

    class Echo(fixture_pb2_grpc.EchoServiceServicer):
        def Call(self, request, context):
            assert request == expected
            observations.append('unary')
            return request

        def Watch(self, request, context):
            assert request == expected
            observations.append('server')
            yield request
            yield other

        def Collect(self, requests, context):
            values = list(requests)
            assert values == [expected, other]
            observations.append('client')
            return other

        def Chat(self, requests, context):
            values = []
            for request in requests:
                values.append(request)
                yield request
            assert values == [expected, other]
            observations.append('bidi')

    with ThreadPoolExecutor(max_workers=2) as pool:
        server = grpc.server(pool)
        fixture_pb2_grpc.add_EchoServiceServicer_to_server(Echo(), server)
        port = server.add_insecure_port('127.0.0.1:0')
        assert port
        server.start()
        try:
            run([str(binary), f'127.0.0.1:{port}'], consumer, timeout=120)
            assert observations == ['unary', 'server', 'client', 'bidi'], observations
        finally:
            server.stop(0).wait(timeout=10)
    assert all(hashlib.sha256((root / path).read_bytes()).hexdigest() == digest
               for path, digest in before.items()), 'Generated Objective-C sources changed'
    evidence = {'target': 'protobuf-objc', 'representation': root.name, 'engine': manifest['engine'],
                'platform': platform.platform(), 'architecture': platform.machine(),
                'xcode': subprocess.check_output(['xcodebuild', '-version'], text=True).strip(),
                'cocoapods': subprocess.check_output(['pod', '--version'], text=True).strip(),
                'grpc': '1.84.0', 'protobuf': '5.36.1', 'python_server_grpc': grpc.__version__,
                'modes': observations, 'descriptor_sha256': descriptor,
                'generated_source_sha256': before,
                'pod_lock_sha256': hashlib.sha256((consumer / 'Podfile.lock').read_bytes()).hexdigest(),
                'result': 'passed'}
    (root / 'runtime-evidence.json').write_text(json.dumps(evidence, indent=2) + '\n')
    print(json.dumps(evidence, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    parser.add_argument('--server-dir', type=Path, required=True)
    verify(parser.parse_args())
