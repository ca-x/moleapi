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

Actual evidence: all18 language targets generated recursive/enum/nullable component models in the host-free Rust VM; the capped application worker produced all20 target artifacts, including actual MySQL/PostgreSQL DDL. Generated Python, JavaScript, TypeScript, Go and C# models ran valid recursive/enum/nullable/Unicode serialization roundtrips; TypeScript, Go and C# were compiled with their real toolchains. Other model language compilation and database execution of generated DDL are not established by artifact generation. Required/optional/null serialization differences remain those of the upstream target; this is model conversion, not universal schema validation.

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
