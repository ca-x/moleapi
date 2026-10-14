# Data model language coverage

Apifox's [model code list](https://docs.apifox.com/generate-code.md), checked2026-10-09, names19 language families. MoleAPI integrates all19 families:18 Quicktype targets and SQL with two OpenAPI Generator dialects.

| Language | Shared target |
| --- | --- |
| C# | model-cs |
| C++ | model-c++ |
| Crystal | model-crystal |
| Dart | model-dart |
| Elm | model-elm |
| Flow | model-flow |
| Go | model-go |
| Haskell | model-haskell |
| Java | model-java |
| JavaScript | model-javascript |
| Kotlin | model-kotlin |
| Objective-C | model-objc |
| Pike | model-pike |
| Python | model-python |
| Ruby | model-ruby |
| Rust | model-rust |
| SQL | mysql-schema, postgresql-schema |
| Swift | model-swift |
| TypeScript | model-typescript |

The specification generation dialog has a separate data-model category, language selection, all/exact component selection and expandable upstream serialization/style settings. Enum choices and booleans retain their types. Preview/copy/file/ZIP downloads, regeneration and owner/content/cancellation guards reuse project artifacts. SQL exports DDL only and requires the existing explicit Java runtime; the18 Quicktype targets work without Java or Node in hosted and offline applications.

Actual evidence: all18 language targets generated recursive/enum/nullable component models in the host-free Rust VM; the capped application worker produced all20 target artifacts, including actual MySQL/PostgreSQL DDL. Generated Python, JavaScript, TypeScript, Go, C#, Java, Rust, Ruby, C++, Kotlin, Dart, Swift, Flow, Elm and Pike models now run recursive/enum/nullable/Unicode serialization roundtrips in OpenAPI3.0.3/3.1.0 fixtures. TypeScript, Go, C#, Java, Rust, C++, Kotlin, Dart, Swift, Flow and Elm compile with real toolchains; Python, JavaScript, Ruby and Pike run through their native runtimes. The other3 model language runtimes and execution of generated SQL DDL are not established by artifact generation. Required/optional/null serialization differences remain those of the upstream target; this is model conversion, not universal schema validation.

API evidence covers authenticated ownership, anonymous/cross-owner rejection, default credential-example exclusion, exact component selection, typed-option rejection and unchanged canonical source. Component/UI tests cover typed model settings and existing late-reply fencing; English/Chinese catalogs include model flows.

Actual agent-browser interaction with the Rust server and Vite frontend imported the canonical definition, selected the separate model category and exact component, generated TypeScript/C# and displayed the emitted files in English/light and Chinese/dark. The README screenshots are those real flows. This does not establish native OS execution. A reproduced Vite development startup failure came from prebundling GraphiQL's worker entry before Vite could transform its ?worker imports; excluding both worker setup entrypoints from optimizeDeps restored startup and actual app interaction.

Reproduce trusted fixtures:

```sh
CARGO_TARGET_DIR=/tmp/moleapi-model-target cargo test -p moleapi-generation --lib project::models::tests
CARGO_TARGET_DIR=/tmp/moleapi-model-target cargo test -p moleapi-server --test generation_models
CARGO_TARGET_DIR=/tmp/moleapi-model-target cargo run -p moleapi-generation --example fixture_models -- /absolute/fixture-output /absolute/moleapi-server /absolute/java
python3 tools/models/verify.py --fixtures /absolute/fixture-output --dotnet /absolute/dotnet --typescript /absolute/typescript/bin/tsc
```

The example builds trusted developer fixtures, not imported programs. Runtime generation never executes target-language programs. Advanced3.1 resource IDs/anchors/dynamic references, JSON Schema dialect compatibility and broad per-target compile/run evidence remain required within the full generation module. SDK/server/custom-template requirements are separate; this does not complete the395-feature objective.


## Java/Rust/Ruby/C++ native model acceptance

The fixture example additionally accepts an explicit dialect and selected model IDs
after the optional Java path. Defaults and output layout stay compatible. Each trusted
fixture retains the exact canonical source bytes alongside the artifact manifest.
The native driver checks source/generator/target/manifest hashes, copies original model
files into separate consumer projects, and verifies the source remains unchanged.

All8 default cases (four languages by two dialects) passed actual decoded and constructed
native-model roundtrips, zero-valued integer, recursive Pet/Status types, authored null,
Unicode with quotes/ampersand/plus and unknown enum rejection. Emitted models match byte
for byte across these equivalent3.0/3.1 schemas. No renderer/serializer patch or model
replacement was needed. Original9-target catalog entries now identify representative
3.0/3.1 native roundtrip evidence; other targets retain engine-integration status.

Recorded Linux combinations: Temurin JDK21.0.12.1+1/Maven3.9.11/Jackson2.17.3;
Cargo1.99.0/Serde1.0.229/serde_json1.0.151 release; Ruby3.4.7/
dry-struct1.8.1/dry-types1.9.1; GCC16.2.1/C++17/nlohmann-json3.12.0. Java dependencies
resolve with strict Maven checksums; Rust retains Cargo.lock/checksum verification;
Ruby gems match published Rubygems hashes with a checksummed Bundler lock; the nlohmann
package signature verifies against the official Arch keyring (signer
109415E692007609CA7EBFE4001CF4810BE8D911). Actual dependency/source/tool/hash and runtime
evidence remains in isolated QA. Product generation never installs these dependencies
or compiles/runs the output.

Existing5 native model languages additionally passed10 fresh3.0/3.1 regression cases.
80 generation regressions, the owned model API case and scoped format/strict Clippy
pass. Broader scalar/required validation, optional/null distinctions in upstream model
representations, advanced unions/resources/constraints, naming/options, different
dependencies/toolchains/platforms and the remaining full generation/release scope stay
open; these serializers are not universal JSON Schema validators.

Reproduce native fixtures and one consumer (repeat for the other dialect/target):

```sh
cargo run -p moleapi-generation --example fixture_models -- /absolute/models-3.1 /absolute/moleapi-server /absolute/java 3.1.0 model-java,model-rust,model-ruby,model-c++
python3 tools/models/verify-native.py --fixtures /absolute/models-3.1 --dialect 3.1.0 --target model-java --jdk /absolute/jdk --maven /absolute/mvn
python3 tools/models/verify-native.py --fixtures /absolute/models-3.1 --dialect 3.1.0 --target model-rust --cargo /absolute/cargo
python3 tools/models/verify-native.py --fixtures /absolute/models-3.1 --dialect 3.1.0 --target model-ruby --ruby /absolute/ruby --gem-home /absolute/locked-gems
python3 tools/models/verify-native.py --fixtures /absolute/models-3.1 --dialect 3.1.0 --target model-c++ --cxx /absolute/g++ --json-include /absolute/nlohmann-header-directory
```

The Ruby gem home must be prepared explicitly with the versions above; the C++ include
directory contains json.hpp and its associated upstream headers. Use fresh fixture
roots; the driver creates a separate consumer directory and refuses to overwrite it.


## Kotlin/Dart/Swift native model acceptance

All6 default3.0.3/3.1.0 cases passed actual decoded and constructed recursive
Pet/Status models, zero-valued ID, nullable native value, Unicode/quotes/ampersand/
plus and unknown-enum rejection. Original emitted sources match across the equivalent
nullable schemas and remain unchanged after compilation. Kotlin uses its default
Jackson mapper with Kotlin2.4.0/Gradle8.14.5/JDK21.0.12.1/Jackson2.17.3; Dart3.13.5
passes analyzer (fatal infos) and executes its AOT binary with SDK-only dependencies;
Swift6.4.1 builds/runs a release SwiftPM executable using Linux Foundation Codable.
Kotlin dependency locks/resolution, Dart lock, source/tool/hash/platform and exact
runtime records stay in isolated QA. Product never fetches or executes these tools.

Default Kotlin Jackson NON_NULL and Swift synthesized Codable omit nil-valued optional
properties when serializing. Both preserve the tested native nullable value; they do
not preserve whether that input field was absent or explicitly null. Dart's default
serializer emits null. The driver asserts each actual output policy and records
nullable_serialization; it retains explicit-null assertions for the earlier four
native targets rather than relaxing their checks. Full optional/null presence
fidelity, required/scalar/constraint validation and more complex shapes remain open
parent-module requirements. These are model conversion cases, not universal schema
validation or a claim of identical input/output JSON object membership.

Fresh Java/Rust/Ruby/C++ driver regressions passed4 further actual cases;80 generation
regressions,1 owned model API case and scoped format/strict Clippy pass. Catalog now
records12 representative model language runtimes. Linux Swift proof does not establish
Apple/iOS deployment; other platforms/toolchains/options/frameworks/dependency ranges,
6 model runtimes, SQL execution, advanced model/SDK/server generation and release gates
remain open.

```sh
cargo run -p moleapi-generation --example fixture_models -- /absolute/three-models /absolute/moleapi-server /absolute/java 3.1.0 model-kotlin,model-dart,model-swift
python3 tools/models/verify-native.py --fixtures /absolute/three-models --dialect 3.1.0 --target model-kotlin --jdk /absolute/jdk --gradle /absolute/gradle
python3 tools/models/verify-native.py --fixtures /absolute/three-models --dialect 3.1.0 --target model-dart --dart /absolute/dart
python3 tools/models/verify-native.py --fixtures /absolute/three-models --dialect 3.1.0 --target model-swift --swift /absolute/swift
```

Repeat using a separate fresh3.0.3 output root. An explicit Swift launcher may set
only its selected toolchain's verified runtime-library paths, as used in Linux QA.


## Flow and recursive Elm compatibility

The pinned mature model engine now repairs reproduced Flow/Elm compiler failures.
Flow uses unknown, ES2022 Object.hasOwn and a Date identity guard compatible with
modern control-flow refinement, retaining the original codecs. Native Flow0.335.0
checks original generated interfaces and typed construction; four core/date cases
(3.0.3/3.1.0) each reject wrong ID/enum construction with2 actual type errors and
reject unknown runtime enum, with native Date/invalid-date checks in date profiles.
Babel8.0.7 / flow-strip-types8.0.1 strips annotations without runtime model rewrites,
and actual Node26.11.1 executes decoded/constructed/zero/recursive/null/Unicode cases.
Flow output now requires modern Flow and ES2022 own-key support; older ranges remain
unverified.

Elm's original recursive record alias fails actual compilation (Elm0.19.3 crashes on
the original and minimal cyclic alias, while a nonrecursive control compiles). Mature
cycle analysis now chooses native tagged record wrappers; standard lazy decoders stop
eager recursion. Wrapper constructors are exported and use independently allocated
argument names that cannot shadow global decoders. Existing property/enum/JSON library
logic remains upstream. Both dialects passed optimized real Elm/Node decode/construction
and unknown-enum cases; a separate nonrecursive alias case and StatusValue global-name
collision case pass. Recursive clients construct Pet { ... } and unwrap (Pet fields);
nonrecursive alias construction remains unchanged.

All8 core/date/alias/collision cases passed on Linux. Exact emitted sources remain
unchanged during verification. Both artifact engines now identify
quicktype-core@26.0.0+moleapi.1; source/license/version/original/patched SHA256s and
idempotent build diffs are in vendor/model-engine/compat.16 other Quicktype target
outputs match original generated bytes, with5 existing native-language regressions
passing;80 generation cases,1 owned model API case and scoped format/strict Clippy pass.
Catalog records14 representative model runtimes.

Elm uses elm/core1.0.5, elm/json1.1.3 and NoRedInk/elm-json-decode-pipeline1.0.0.
Their official registry archive hashes matched the exact GitHub codeload archives
after the compiler's zipball URL timed out. Package/npm locks, integrity/source/tool/
dialect/platform/compiler/runtime and negative checker evidence remains in isolated QA.
Flow runs foreground full-check with bounded workers; Node ports finish and clear the
explicit timeout. Product never downloads or executes these QA dependencies.

The conversion cases do not provide universal schema validation or missing/null
object-membership fidelity. Optional nonnullable fields represented by native Maybe/
Option can serialize an absent child field as null; that policy is not established as
valid for every authored schema and remains required follow-up. Advanced/mutual/
container recursion, schema/resource/dialect options, other framework/toolchain/platform
ranges,4 remaining model runtimes, SQL execution and full SDK/product/release scope
remain open.

```sh
cargo run -p moleapi-generation --example fixture_models -- /absolute/flow-elm /absolute/moleapi-server /absolute/java 3.1.0 model-flow,model-elm
python3 tools/models/verify-flow-elm.py --fixtures /absolute/flow-elm --dialect 3.1.0 --target model-flow --node /absolute/node --flow /absolute/flow --tools /absolute/locked-babel-flow-tools
python3 tools/models/verify-flow-elm.py --fixtures /absolute/flow-elm --dialect 3.1.0 --target model-elm --node /absolute/node --elm /absolute/elm --elm-home /absolute/elm-package-cache
```

Use fresh roots per dialect. Optional final fixture profiles date (model-flow), flat
and collision (model-elm) expose the separately tested dispatch/alias/name cases.
Rebuilding the embedded model engine uses npm --prefix tools/models ci then
npm --prefix tools/models run build; package/version/hash guards prevent patching
an unexpected upstream release.


## Pike recursive and container models

Actual native Pike compilation exposed shallow nested-model decoding and missing
enum checking in the pinned mature renderer. The compatibility patch now delegates
property values to existing generated model/enum conversions and uses native map
for arrays/maps, including top-level containers. Optional missing fields and explicit
JSON null follow original native representation; required enum values reject unknown
strings. Original native class/member/JSON/primitive handling remains in control.
No generated model rewrites, custom JSON parser or alternate object substitute occurs.

Four real3.0.3/3.1.0 core/container cases pass decoded and constructed native models,
recursive objects, zero, known/unknown enum, explicit null and Unicode with quotes/
ampersand/plus. Containers cover direct models, two model-array entries, Unicode and
zero-prefixed string map keys, enum arrays with rejection, integer-map zero and top-level
model arrays/maps. The consumer asserts nested objects rather than retained mappings,
and checks original file hashes and exact source/dialect/manifest. Core model sources
match across the equivalent schemas; artifact engine is26.0.0+moleapi.1.

Runtime: Pike8.0 release1956, isolated Ubuntu8.0.1956-4 native core package on Linux
with official HTTPS package-index/archive SHA256 match. An isolated libcrypt compatibility
package verifies against the official Arch keyring, with its exact package/signer/hash
record. The explicit launcher sets only that runtime's master/module/include/library
paths; system packages remain unchanged. Standard utf8_to_string/string_to_utf8 wrap
QA file/console I/O, preserving actual Unicode values without changing model codecs.
Tool versions include stderr because native Pike prints --version there. Exact tool/
package/source/hash/platform/runtime evidence remains in isolated QA. Product generation
never downloads or executes this runtime.

17 other Quicktype target outputs match their pre-patch bytes;5 prior model runtime
regressions,80 generation cases,1 owned model API case and scoped format/strict Clippy
pass. Catalog now records15 representative model runtimes. Other Pike versions/platforms/
containers/options, complex/ambiguous unions, general scalar/schema validation and
missing/null membership fidelity remain required work. Crystal/Haskell/Objective-C
model runtimes, SQL execution and full SDK/product/platform/release scope remain open.

```sh
cargo run -p moleapi-generation --example fixture_models -- /absolute/pike-models /absolute/moleapi-server /absolute/java 3.1.0 model-pike
python3 tools/models/verify-pike.py --fixtures /absolute/pike-models --dialect 3.1.0 --pike /absolute/pike-launcher
```

Use a separate fresh root per dialect and append containers as the fixture example's
final argument for the dedicated container cases. The explicit launcher may configure
its isolated library and Pike paths, without changing host/runtime settings.
