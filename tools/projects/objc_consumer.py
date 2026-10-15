"""Prepare a private, sequential Apple QA consumer without rewriting generated clients."""
import hashlib
import json
from pathlib import Path

PODFILE = """source 'https://cdn.cocoapods.org/'
platform :osx, '12.0'
project 'FixtureQA.xcodeproj'
target 'FixtureQA' do
  pod 'gRPC-ProtoRPC', '1.84.0'
  pod 'Protobuf', '5.36.1'
end
"""


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def native_arch(machine):
    if machine not in ('arm64', 'x86_64'):
        raise ValueError('Apple QA requires an actual arm64 or x86_64 host')
    return machine


def build_command(consumer, architecture):
    native_arch(architecture)
    return ['xcodebuild', '-workspace', 'FixtureQA.xcworkspace', '-scheme', 'FixtureQA',
            '-configuration', 'Release', '-derivedDataPath', str(consumer / 'build'),
            '-destination', f'platform=macOS,arch={architecture}', f'ARCHS={architecture}',
            'ONLY_ACTIVE_ARCH=YES', 'CODE_SIGNING_ALLOWED=NO', 'build']


def pod_state(consumer):
    lock = consumer / 'Podfile.lock'
    manifest = consumer / 'Pods/Manifest.lock'
    if not lock.is_file() or not manifest.is_file() or lock.read_bytes() != manifest.read_bytes():
        raise ValueError('Native QA Podfile.lock and installed Pods manifest differ')
    text = lock.read_text()
    if 'gRPC-ProtoRPC (1.84.0)' not in text or 'Protobuf (5.36.1)' not in text:
        raise ValueError('Native QA requires the pinned gRPC/Protobuf versions')
    return {'pod_lock_sha256': digest(lock),
            'pod_project_sha256': digest(consumer / 'Pods/Pods.xcodeproj/project.pbxproj')}


def prepare_consumer(root, consumer, scripts, architecture, xcode, descriptor, run):
    root, consumer, scripts = root.resolve(), consumer.resolve(), scripts.resolve()
    if consumer.is_relative_to(root) and consumer != root / 'runtime-consumer':
        raise ValueError('A shared consumer must be outside the generated-source root')
    fingerprint = {'version': 1, 'architecture': native_arch(architecture), 'xcode': xcode,
                   'descriptor_sha256': descriptor,
                   'podfile_sha256': hashlib.sha256(PODFILE.encode()).hexdigest(),
                   'scripts': {name: digest(scripts / name) for name in ('create-project.rb', 'RuntimeFixture.m')}}
    marker = consumer / '.moleapi-objc-consumer.json'
    reused = consumer.exists()
    if reused:
        if not marker.is_file():
            raise ValueError('Refusing an unowned or incomplete native QA consumer directory')
        state = json.loads(marker.read_text())
        if state.get('fingerprint') != fingerprint:
            raise ValueError('Native QA consumer fingerprint differs; select a fresh directory')
        if (consumer / 'Podfile').read_text() != PODFILE or (consumer / 'RuntimeFixture.m').read_bytes() != (scripts / 'RuntimeFixture.m').read_bytes():
            raise ValueError('Native QA consumer inputs changed')
        if (consumer / 'LinkRuntime.swift').read_text() != '// Link the selected Xcode Swift runtime required by Protobuf.\n':
            raise ValueError('Native QA Swift linkage source changed')
        if state.get('pods') != pod_state(consumer):
            raise ValueError('Native QA dependency lock/project changed')
    else:
        consumer.mkdir(parents=True, mode=0o700)
        (consumer / 'RuntimeFixture.m').write_bytes((scripts / 'RuntimeFixture.m').read_bytes())
        (consumer / 'Podfile').write_text(PODFILE)
    # The stable project keeps Pods/target identity; only original-source references switch.
    run(['ruby', str(scripts / 'create-project.rb'), str(root), str(consumer)], consumer)
    if not reused:
        run(['pod', 'install'], consumer)
        state = {'fingerprint': fingerprint, 'pods': pod_state(consumer)}
        marker.write_text(json.dumps(state, indent=2) + '\n')
    return consumer, reused, state['pods']
