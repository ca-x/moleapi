#!/usr/bin/env python3
"""Compile/run original Quicktype Java/Rust/Ruby/C++ outputs; explicit trusted QA only."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess

NOTE = '鼹鼠 "quotes" & +'
SAMPLE = {'id': 0, 'status': 'ready', 'note': None,
          'friend': {'id': 2, 'status': 'done', 'note': NOTE}}

JAVA = r'''package fixture;
import io.quicktype.*;
import java.nio.file.*;
public class RuntimeFixture {
  static void ensure(boolean value) {if (!value) throw new IllegalStateException("Native model fields lost");}
  public static void main(String[] args) throws Exception {
    String source = Files.readString(Path.of("sample.json"));
    Pet decoded = Converter.fromJsonString(source);
    ensure(decoded.getID() == 0 && decoded.getStatus() == Status.READY && decoded.getNote() == null);
    ensure(decoded.getFriend() instanceof Pet && decoded.getFriend().getID() == 2 && decoded.getFriend().getStatus() == Status.DONE);
    ensure(decoded.getFriend().getNote().equals("鼹鼠 \"quotes\" & +"));
    System.out.println(Converter.toJsonString(decoded));
    Pet child = new Pet(); child.setID(2); child.setStatus(Status.DONE); child.setNote("鼹鼠 \"quotes\" & +");
    Pet built = new Pet(); built.setID(0); built.setStatus(Status.READY); built.setNote(null); built.setFriend(child);
    System.out.println(Converter.toJsonString(built));
    boolean rejected = false;
    try {Converter.fromJsonString(source.replace("\"ready\"", "\"invalid\""));} catch (java.io.IOException expected) {rejected = true;}
    ensure(rejected);
  }
}
'''

POM = '''<project xmlns="http://maven.apache.org/POM/4.0.0"><modelVersion>4.0.0</modelVersion>
<groupId>fixture</groupId><artifactId>model-qa</artifactId><version>1</version>
<properties><maven.compiler.release>17</maven.compiler.release><project.build.sourceEncoding>UTF-8</project.build.sourceEncoding></properties>
<dependencies>
<dependency><groupId>com.fasterxml.jackson.core</groupId><artifactId>jackson-databind</artifactId><version>2.17.3</version></dependency>
<dependency><groupId>com.fasterxml.jackson.datatype</groupId><artifactId>jackson-datatype-jsr310</artifactId><version>2.17.3</version></dependency>
</dependencies>
<build><plugins>
<plugin><groupId>org.apache.maven.plugins</groupId><artifactId>maven-compiler-plugin</artifactId><version>3.14.0</version></plugin>
<plugin><groupId>org.apache.maven.plugins</groupId><artifactId>maven-dependency-plugin</artifactId><version>3.8.1</version></plugin>
</plugins></build></project>
'''

RUST = r'''mod models;
use models::{Pet, Status};
fn main() {
    let source = std::fs::read_to_string("sample.json").unwrap();
    let decoded: Pet = serde_json::from_str(&source).unwrap();
    assert_eq!(decoded.id, 0); assert!(matches!(decoded.status, Status::Ready)); assert!(decoded.note.is_none());
    let child = decoded.friend.as_ref().unwrap();
    assert_eq!(child.id, 2); assert!(matches!(child.status, Status::Done));
    assert_eq!(child.note.as_deref(), Some("鼹鼠 \"quotes\" & +"));
    println!("{}", serde_json::to_string(&decoded).unwrap());
    let built = Pet { id: 0, status: Status::Ready, note: None,
        friend: Some(Box::new(Pet { id: 2, status: Status::Done, note: Some("鼹鼠 \"quotes\" & +".into()), friend: None })) };
    println!("{}", serde_json::to_string(&built).unwrap());
    assert!(serde_json::from_str::<Pet>(&source.replace("\"ready\"", "\"invalid\"")).is_err());
}
'''

CARGO = '''[package]
name = "model-qa"
version = "0.1.0"
edition = "2024"
[workspace]
[dependencies]
serde = {version = "=1.0.229", features = ["derive"]}
serde_json = "=1.0.151"
'''

RUBY = r'''require_relative 'models'
def ensure_fixture(value)
  raise 'Native model fields lost' unless value
end
source = File.read('sample.json', encoding: 'UTF-8')
decoded = Pet.from_json!(source)
ensure_fixture(decoded.id == 0 && decoded.status == Status::Ready && decoded.note.nil?)
ensure_fixture(decoded.friend.is_a?(Pet) && decoded.friend.id == 2 && decoded.friend.status == Status::Done)
ensure_fixture(decoded.friend.note == '鼹鼠 "quotes" & +')
puts decoded.to_json
child = Pet.new(id: 2, status: Status::Done, note: '鼹鼠 "quotes" & +', friend: nil)
built = Pet.new(id: 0, status: Status::Ready, note: nil, friend: child)
puts built.to_json
rejected = false
begin
  Pet.from_json!(source.sub('"ready"', '"invalid"'))
rescue StandardError
  rejected = true
end
ensure_fixture(rejected)
File.write('runtime-gems.json', JSON.pretty_generate(Gem.loaded_specs.transform_values { |spec| spec.version.to_s }))
'''

CPP = r'''#include <fstream>
#include <iostream>
#include <memory>
#include <stdexcept>
#include "models.cpp"
void ensure(bool value) {if (!value) throw std::runtime_error("Native model fields lost");}
int main() {
    std::ifstream source("sample.json");
    nlohmann::json wire; source >> wire;
    auto decoded = wire.get<quicktype::Pet>();
    ensure(decoded.get_id() == 0 && decoded.get_status() == quicktype::Status::READY && !decoded.get_note());
    auto child = decoded.get_pet_friend();
    ensure(child && child->get_id() == 2 && child->get_status() == quicktype::Status::DONE);
    ensure(child->get_note() == std::optional<std::string>("鼹鼠 \"quotes\" & +"));
    std::cout << nlohmann::json(decoded).dump() << '\n';
    auto built_child = std::make_shared<quicktype::Pet>();
    built_child->set_id(2); built_child->set_status(quicktype::Status::DONE); built_child->set_note("鼹鼠 \"quotes\" & +");
    quicktype::Pet built; built.set_id(0); built.set_status(quicktype::Status::READY); built.set_note(std::nullopt); built.set_pet_friend(built_child);
    std::cout << nlohmann::json(built).dump() << '\n';
    wire["status"] = "invalid"; bool rejected = false;
    try {wire.get<quicktype::Pet>();} catch (const std::exception&) {rejected = true;}
    ensure(rejected);
}
'''

KOTLIN = r'''package fixture
import quicktype.*
import java.nio.file.*
fun main() {
    val source = Files.readString(Path.of("sample.json"))
    val decoded = Pet.fromJson(source)
    check(decoded.id == 0L && decoded.status == Status.Ready && decoded.note == null)
    val child = requireNotNull(decoded.friend)
    check(child.id == 2L && child.status == Status.Done && child.note == "鼹鼠 \"quotes\" & +")
    println(decoded.toJson())
    val built = Pet(id = 0L, status = Status.Ready, note = null,
        friend = Pet(id = 2L, status = Status.Done, note = "鼹鼠 \"quotes\" & +"))
    println(built.toJson())
    var rejected = false
    try {Pet.fromJson(source.replace("\"ready\"", "\"invalid\""))} catch (expected: Exception) {rejected = true}
    check(rejected)
}
'''

GRADLE = '''plugins { kotlin("jvm") version "2.4.0"; application }
repositories { mavenCentral() }
dependencies {
    implementation("com.fasterxml.jackson.module:jackson-module-kotlin:2.17.3")
    implementation(kotlin("reflect"))
}
kotlin { jvmToolchain(21) }
application { mainClass.set("fixture.RuntimeFixtureKt") }
dependencyLocking { lockAllConfigurations() }
'''

DART = r'''import 'dart:convert';
import 'dart:io';
import 'models.dart';
void ensure(bool value) { if (!value) throw StateError('Native model fields lost'); }
void main() {
    final source = File('sample.json').readAsStringSync();
    final decoded = petFromJson(source);
    ensure(decoded.id == 0 && decoded.status == Status.READY && decoded.note == null);
    final child = decoded.friend!;
    ensure(child.id == 2 && child.status == Status.DONE && child.note == '鼹鼠 "quotes" & +');
    print(petToJson(decoded));
    final built = Pet(id: 0, status: Status.READY, note: null,
        friend: Pet(id: 2, status: Status.DONE, note: '鼹鼠 "quotes" & +'));
    print(petToJson(built));
    final invalid = jsonDecode(source) as Map<String, dynamic>;
    invalid['status'] = 'invalid';
    var rejected = false;
    try {petFromJson(jsonEncode(invalid));} catch (_) {rejected = true;}
    ensure(rejected);
}
'''

SWIFT = r'''import Foundation
func ensure(_ value: Bool) { if !value { fatalError("Native model fields lost") } }
let source = try String(contentsOfFile: "sample.json", encoding: .utf8)
let decoded = try Pet(source)
ensure(decoded.id == 0 && decoded.status == .ready && decoded.note == nil)
let child = decoded.friend!
ensure(child.id == 2 && child.status == .done && child.note == "鼹鼠 \"quotes\" & +")
print(try decoded.jsonString()!)
let built = Pet(friend: Pet(friend: nil, id: 2, note: "鼹鼠 \"quotes\" & +", status: .done),
    id: 0, note: nil, status: .ready)
print(try built.jsonString()!)
var rejected = false
do { _ = try Pet(source.replacingOccurrences(of: "\"ready\"", with: "\"invalid\"")) }
catch { rejected = true }
ensure(rejected)
'''

SWIFT_PACKAGE = '''// swift-tools-version: 6.0
import PackageDescription
let package = Package(name: "ModelQA", targets: [.executableTarget(name: "ModelQA")])
'''


def run(command, cwd, env=None, timeout=600):
    print('RUN', ' '.join(map(str, command)), flush=True)
    result = subprocess.run(command, cwd=cwd, env=env, text=True, capture_output=True, timeout=timeout)
    if result.returncode:
        raise RuntimeError(result.stdout + result.stderr)
    return result.stdout


def check(output, nullable_serialization='null'):
    records = [json.loads(line) for line in output.splitlines() if line.startswith('{')]
    assert len(records) == 2, output
    for value in records:
        assert value['id'] == 0 and value['status'] == 'ready'
        if nullable_serialization == 'omitted':
            assert 'note' not in value
        else:
            assert 'note' in value and value['note'] is None
        assert value['friend']['id'] == 2 and value['friend']['status'] == 'done'
        assert value['friend']['note'] == NOTE


def verify(args):
    root = args.fixtures
    assert root.is_absolute() and root.is_dir()
    target = args.target
    sources = root / target
    manifest = json.loads((sources / 'moleapi-generation.json').read_text())
    assert manifest['target'] == target and manifest['engine'] == 'quicktype-core@26.0.0'
    specification = (sources / 'fixture-source.json').read_bytes()
    assert json.loads(specification)['openapi'] == args.dialect
    assert hashlib.sha256(specification).hexdigest() == manifest['source_sha256']
    before = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sources.iterdir() if p.is_file() and p.name != 'moleapi-generation.json'}
    assert all(before[file['path']] == file['sha256'] for file in manifest['files'])
    consumer = root / f'{target}-consumer'
    consumer.mkdir(exist_ok=False)
    (consumer / 'sample.json').write_text(json.dumps(SAMPLE, ensure_ascii=False) + '\n')
    tools = {}
    dependencies = {}
    nullable_serialization = 'null'
    if target == 'model-java':
        assert args.jdk and args.jdk.is_absolute() and (args.jdk / 'bin/java').is_file()
        assert args.maven and args.maven.is_absolute() and args.maven.is_file()
        env = dict(os.environ, JAVA_HOME=str(args.jdk), PATH=str(args.jdk / 'bin') + os.pathsep + os.environ['PATH'])
        java = consumer / 'src/main/java/io/quicktype'
        java.mkdir(parents=True)
        for path in sources.glob('*.java'):
            shutil.copyfile(path, java / path.name)
        driver = consumer / 'src/main/java/fixture'
        driver.mkdir()
        (driver / 'RuntimeFixture.java').write_text(JAVA)
        (consumer / 'pom.xml').write_text(POM)
        run([str(args.maven), '--strict-checksums', '-B', 'package', 'dependency:copy-dependencies', '-DincludeScope=runtime'], consumer, env)
        output = run([str(args.jdk / 'bin/java'), '-cp', f'target/classes{os.pathsep}target/dependency/*', 'fixture.RuntimeFixture'], consumer, env)
        tools['java'] = run([str(args.jdk / 'bin/javac'), '-version'], consumer).strip()
        dependencies = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in (consumer / 'target/dependency').glob('*.jar')}
    elif target == 'model-rust':
        assert args.cargo and args.cargo.is_absolute() and args.cargo.is_file()
        (consumer / 'src').mkdir()
        shutil.copyfile(sources / 'models.rs', consumer / 'src/models.rs')
        (consumer / 'src/main.rs').write_text(RUST)
        (consumer / 'Cargo.toml').write_text(CARGO)
        run([str(args.cargo), 'generate-lockfile'], consumer)
        output = run([str(args.cargo), 'run', '--locked', '--quiet', '--release'], consumer)
        tools['cargo'] = run([str(args.cargo), '--version'], consumer).strip()
        dependencies['cargo_lock_sha256'] = hashlib.sha256((consumer / 'Cargo.lock').read_bytes()).hexdigest()
    elif target == 'model-ruby':
        assert args.ruby and args.ruby.is_absolute() and args.ruby.is_file()
        assert args.gem_home and args.gem_home.is_absolute() and args.gem_home.is_dir()
        default = run([str(args.ruby), '-rrubygems', '-e', 'print Gem.default_dir'], consumer)
        env = dict(os.environ, GEM_HOME=str(args.gem_home), GEM_PATH=str(args.gem_home) + os.pathsep + default)
        shutil.copyfile(sources / 'models.rb', consumer / 'models.rb')
        (consumer / 'RuntimeFixture.rb').write_text(RUBY)
        output = run([str(args.ruby), 'RuntimeFixture.rb'], consumer, env)
        tools['ruby'] = run([str(args.ruby), '--version'], consumer).strip()
        dependencies = json.loads((consumer / 'runtime-gems.json').read_text())
    elif target == 'model-c++':
        assert target == 'model-c++'
        assert args.cxx and args.cxx.is_absolute() and args.cxx.is_file()
        assert args.json_include and args.json_include.is_absolute() and (args.json_include / 'json.hpp').is_file()
        shutil.copyfile(sources / 'models.cpp', consumer / 'models.cpp')
        (consumer / 'RuntimeFixture.cpp').write_text(CPP)
        run([str(args.cxx), '-std=c++17', '-O2', '-I' + str(args.json_include), '-I' + str(args.json_include.parent), 'RuntimeFixture.cpp', '-o', 'model-qa'], consumer)
        output = run([str(consumer / 'model-qa')], consumer)
        tools['cxx'] = run([str(args.cxx), '--version'], consumer).splitlines()[0]
        dependencies['nlohmann_header_sha256'] = hashlib.sha256((args.json_include / 'json.hpp').read_bytes()).hexdigest()
    elif target == 'model-kotlin':
        assert args.jdk and args.jdk.is_absolute() and (args.jdk / 'bin/java').is_file()
        assert args.gradle and args.gradle.is_absolute() and args.gradle.is_file()
        env = dict(os.environ, JAVA_HOME=str(args.jdk), PATH=str(args.jdk / 'bin') + os.pathsep + os.environ['PATH'])
        source = consumer / 'src/main/kotlin'
        source.mkdir(parents=True)
        shutil.copyfile(sources / 'models.kt', source / 'models.kt')
        (source / 'RuntimeFixture.kt').write_text(KOTLIN)
        (consumer / 'build.gradle.kts').write_text(GRADLE)
        (consumer / 'settings.gradle.kts').write_text('rootProject.name = "model-qa"\n')
        run([str(args.gradle), '--no-daemon', '--write-locks', 'installDist'], consumer, env, timeout=900)
        output = run([str(consumer / 'build/install/model-qa/bin/model-qa')], consumer, env)
        tools['gradle'] = run([str(args.gradle), '--version'], consumer, env).strip()
        tools['java'] = run([str(args.jdk / 'bin/javac'), '-version'], consumer).strip()
        resolved = run([str(args.gradle), '--no-daemon', 'dependencies', '--configuration', 'runtimeClasspath'], consumer, env)
        (consumer / 'runtime-dependencies.txt').write_text(resolved)
        dependencies['gradle_lock_sha256'] = hashlib.sha256((consumer / 'gradle.lockfile').read_bytes()).hexdigest()
        nullable_serialization = 'omitted'
    elif target == 'model-dart':
        assert args.dart and args.dart.is_absolute() and args.dart.is_file()
        shutil.copyfile(sources / 'models.dart', consumer / 'models.dart')
        (consumer / 'RuntimeFixture.dart').write_text(DART)
        (consumer / 'pubspec.yaml').write_text("name: model_qa\nenvironment:\n  sdk: '>=3.0.0 <4.0.0'\n")
        run([str(args.dart), 'pub', 'get', '--offline'], consumer)
        run([str(args.dart), 'analyze', '--fatal-infos'], consumer)
        run([str(args.dart), 'compile', 'exe', 'RuntimeFixture.dart', '-o', 'model-qa'], consumer)
        output = run([str(consumer / 'model-qa')], consumer)
        tools['dart'] = run([str(args.dart), '--version'], consumer).strip()
        dependencies['pub_lock_sha256'] = hashlib.sha256((consumer / 'pubspec.lock').read_bytes()).hexdigest()
    else:
        assert target == 'model-swift'
        assert args.swift and args.swift.is_absolute() and args.swift.is_file()
        source = consumer / 'Sources/ModelQA'
        source.mkdir(parents=True)
        shutil.copyfile(sources / 'models.swift', source / 'models.swift')
        (source / 'main.swift').write_text(SWIFT)
        (consumer / 'Package.swift').write_text(SWIFT_PACKAGE)
        run([str(args.swift), 'build', '--configuration', 'release'], consumer)
        output = run([str(args.swift), 'run', '--skip-build', '--configuration', 'release', 'ModelQA'], consumer)
        tools['swift'] = run([str(args.swift), '--version'], consumer).strip()
        nullable_serialization = 'omitted'
    check(output, nullable_serialization)
    assert all(hashlib.sha256((sources / path).read_bytes()).hexdigest() == digest for path, digest in before.items())
    evidence = {'target': target, 'dialect': args.dialect, 'engine': manifest['engine'],
                'source_sha256': manifest['source_sha256'], 'platform': platform.platform(),
                'tools': tools, 'dependencies': dependencies, 'generated_source_sha256': before,
                'profiles': ['decoded', 'constructed', 'zero', 'recursive', 'enum', 'nullable', 'unicode', 'invalid-enum'],
                'nullable_serialization': nullable_serialization, 'result': 'passed'}
    (consumer / 'runtime-evidence.json').write_text(json.dumps(evidence, indent=2) + '\n')
    print(f'PASS {target}/{args.dialect}: original native models')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixtures', required=True, type=Path)
    parser.add_argument('--dialect', required=True, choices=['3.0.3', '3.1.0'])
    parser.add_argument('--target', required=True, choices=['model-java', 'model-rust', 'model-ruby', 'model-c++', 'model-kotlin', 'model-dart', 'model-swift'])
    for name in ['jdk', 'maven', 'cargo', 'ruby', 'gem-home', 'cxx', 'json-include', 'gradle', 'dart', 'swift']:
        parser.add_argument('--' + name, type=Path)
    verify(parser.parse_args())
