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

Actual evidence: all18 language targets generated recursive/enum/nullable component models in the host-free Rust VM; the capped application worker produced all20 target artifacts, including actual MySQL/PostgreSQL DDL. Generated Python, JavaScript, TypeScript, Go, C#, Java, Rust, Ruby and C++ models now run recursive/enum/nullable/Unicode serialization roundtrips in OpenAPI3.0.3/3.1.0 fixtures. TypeScript, Go, C#, Java, Rust and C++ compile with real toolchains; Python, JavaScript and Ruby run through their native runtimes. The other9 model language runtimes and execution of generated SQL DDL are not established by artifact generation. Required/optional/null serialization differences remain those of the upstream target; this is model conversion, not universal schema validation.

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
