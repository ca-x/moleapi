#!/usr/bin/env python3
"""Local orchestration/PBX checks; never a substitute for native Apple RPC evidence."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

from objc_consumer import PODFILE, build_command, prepare_consumer

SCRIPTS = Path(__file__).resolve().parent / 'objc'
LOCK = 'PODS:\n  - gRPC-ProtoRPC (1.84.0)\n  - Protobuf (5.36.1)\n'
RUBY = None


class ConsumerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='objc consumer space-')
        self.root = Path(self.temp.name)
        self.source = self.root / 'source'
        self.descriptor = self.root / 'descriptor'
        for path in (self.source, self.descriptor):
            (path / 'models').mkdir(parents=True)
            (path / 'Fixture.pbrpc.h').write_text('// original service header\n')
            (path / 'Fixture.pbrpc.m').write_text('// original service '+path.name+'\n')
            (path / 'models/Types.pbobjc.m').write_text('// original GPB '+path.name+'\n')
            (path / 'models/Types.pbobjc.h').write_text('// original GPB header\n')
        self.consumer = self.root / 'shared'
        self.calls = []

    def tearDown(self):
        self.temp.cleanup()

    def run_tool(self, command, cwd):
        self.calls.append(command)
        if command[0] == 'ruby':
            subprocess.run([str(RUBY), *command[1:]], cwd=cwd, check=True, capture_output=True, text=True)
        else:
            self.assertEqual(command, ['pod', 'install'])
            # A lock-shaped fixture tests orchestration only. No native dependency or RPC pass is inferred.
            (cwd / 'Pods/Pods.xcodeproj').mkdir(parents=True)
            (cwd / 'Podfile.lock').write_text(LOCK)
            (cwd / 'Pods/Manifest.lock').write_text(LOCK)
            (cwd / 'Pods/Pods.xcodeproj/project.pbxproj').write_text('pinned Pods fixture graph\n')
            # Real xcodeproj edits emulate integration identity that must survive source switches.
            program = '''require 'xcodeproj';p=Xcodeproj::Project.open(ARGV[0]);t=p.targets.find{|x|x.name=='FixtureQA'};t.build_configurations.each{|c|c.base_configuration_reference=p.main_group.new_file('Pods/native-'+c.name+'.xcconfig')};p.save'''
            subprocess.run([str(RUBY), '-e', program, str(cwd / 'FixtureQA.xcodeproj')], check=True)

    def prepare(self, source=None, **overrides):
        data = dict(root=source or self.source, consumer=self.consumer, scripts=SCRIPTS,
                    architecture='x86_64', xcode='Xcode fixture', descriptor='same-descriptor', run=self.run_tool)
        data.update(overrides)
        return prepare_consumer(**data)

    def inspect(self):
        program = '''require 'json';require 'xcodeproj';p=Xcodeproj::Project.open(ARGV[0]);t=p.targets.find{|x|x.name=='FixtureQA'};g=p.main_group.children.find{|x|x.isa=='PBXGroup'&&x.name=='Generated'};puts JSON.generate({project:p.root_object.uuid,target:t.uuid,group:g.uuid,generated:g.files.map{|f|f.real_path.to_s},sources:t.source_build_phase.files.map{|f|[f.file_ref.real_path.to_s,f.settings]},configs:t.build_configurations.map{|c|[c.name,c.base_configuration_reference.uuid,c.build_settings['HEADER_SEARCH_PATHS']]}})'''
        return json.loads(subprocess.check_output([str(RUBY), '-e', program, str(self.consumer / 'FixtureQA.xcodeproj')], text=True))

    def test_shared_project_switches_only_original_sources_and_bootstraps_pods_once(self):
        originals = {p: hashlib.sha256(p.read_bytes()).hexdigest() for root in (self.source,self.descriptor) for p in root.rglob('*') if p.is_file()}
        _, reused, pods = self.prepare()
        self.assertFalse(reused)
        first = self.inspect()
        swift = self.consumer / 'LinkRuntime.swift'
        swift_time = swift.stat().st_mtime_ns
        _, reused, next_pods = self.prepare(self.descriptor)
        self.assertTrue(reused)
        second = self.inspect()
        for field in ('project', 'target', 'group'):
            self.assertEqual(first[field], second[field])
        self.assertEqual([c[:2] for c in first['configs']], [c[:2] for c in second['configs']])
        self.assertTrue(all(c[2] == ['$(inherited)', str(self.descriptor)] for c in second['configs']))
        self.assertEqual(pods, next_pods)
        self.assertEqual(sum(c[0] == 'pod' for c in self.calls), 1)
        self.assertEqual(set(second['generated']), {str(p) for p in self.descriptor.rglob('*') if p.suffix in ('.h','.m')})
        paths = [entry[0] for entry in second['sources']]
        self.assertEqual(len(paths), 4)
        self.assertTrue(all(str(self.source) not in p for p in paths))
        self.assertEqual(swift.stat().st_mtime_ns, swift_time)
        for path, flags in second['sources']:
            if path.endswith('.pbobjc.m'): self.assertEqual(flags, {'COMPILER_FLAGS':'-fno-objc-arc'})
        self.prepare(self.descriptor)
        self.assertEqual(sum(c[0] == 'pod' for c in self.calls), 1)
        self.assertEqual(len(self.inspect()['sources']), 4)
        self.assertTrue(all(hashlib.sha256(p.read_bytes()).hexdigest() == digest for p,digest in originals.items()))

    def test_foreign_directory_is_refused_without_mutation(self):
        self.consumer.mkdir()
        keep = self.consumer / 'keep.txt'
        keep.write_text('keep')
        with self.assertRaisesRegex(ValueError, 'unowned'):
            self.prepare()
        self.assertEqual(keep.read_text(), 'keep')
        self.assertFalse(self.calls)

    def test_schema_toolchain_and_script_changes_refuse_reuse_before_switching(self):
        self.prepare()
        project = self.consumer / 'FixtureQA.xcodeproj/project.pbxproj'
        before = project.read_bytes()
        count = len(self.calls)
        for change in ({'architecture':'arm64'}, {'xcode':'another Xcode'}, {'descriptor':'another schema'}):
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, 'fingerprint'):
                self.prepare(self.descriptor, **change)
        scripts = self.root / 'scripts'
        shutil.copytree(SCRIPTS, scripts)
        with (scripts / 'RuntimeFixture.m').open('a') as out: out.write('// changed QA contract\n')
        with self.assertRaisesRegex(ValueError, 'fingerprint'):
            self.prepare(self.descriptor, scripts=scripts)
        self.assertEqual(len(self.calls), count)
        self.assertEqual(project.read_bytes(), before)

    def test_lock_manifest_and_pod_graph_drift_are_refused(self):
        self.prepare()
        for relative in ['Pods/Manifest.lock','Pods/Pods.xcodeproj/project.pbxproj','RuntimeFixture.m','Podfile','LinkRuntime.swift']:
            path = self.consumer / relative
            before = path.read_bytes()
            path.write_bytes(before+b'changed\n')
            count = len(self.calls)
            with self.subTest(relative=relative), self.assertRaises(ValueError): self.prepare(self.descriptor)
            self.assertEqual(len(self.calls),count)
            path.write_bytes(before)

    def test_failed_bootstrap_is_not_reused_as_a_completed_cache(self):
        def interrupted(command,cwd):
            self.run_tool(command,cwd)
            if command[0]=='pod': (cwd/'Pods/Manifest.lock').write_text('incomplete installation')
        with self.assertRaisesRegex(ValueError,'manifest differ'): self.prepare(run=interrupted)
        self.assertFalse((self.consumer/'.moleapi-objc-consumer.json').exists())
        with self.assertRaisesRegex(ValueError,'incomplete'): self.prepare(self.descriptor)

    def test_invalid_hosts_and_nested_shared_paths_are_refused(self):
        for machine in ['AMD64','aarch64','unknown']:
            with self.assertRaises(ValueError): build_command(self.consumer,machine)
        with self.assertRaises(ValueError): self.prepare(consumer=self.source/'cache')
        self.assertFalse((self.source/'cache').exists())

    def test_release_commands_select_exactly_the_executing_host_architecture(self):
        for architecture in ['arm64','x86_64']:
            command = build_command(self.consumer,architecture)
            self.assertEqual(command[command.index('-configuration')+1], 'Release')
            self.assertIn(f'ARCHS={architecture}',command)
            self.assertIn(f'platform=macOS,arch={architecture}',command)
            self.assertIn('ONLY_ACTIVE_ARCH=YES',command)
            self.assertIn('CODE_SIGNING_ALLOWED=NO',command)
            self.assertEqual(command[command.index('-derivedDataPath')+1],str(self.consumer/'build'))


if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--ruby',type=Path,required=True)
    args=parser.parse_args()
    RUBY=args.ruby
    assert RUBY.is_absolute() and RUBY.is_file()
    unittest.main(argv=[__file__],verbosity=2)
