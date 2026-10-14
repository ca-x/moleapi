# Non-Rust protobuf reachable import outputs

Parent: milestone 2 / code-generation.md / PROJECT-GENERATION-COVERAGE.md.

Current proto-source entry_files are only passed as protoc generation targets, leaving
provided imported model outputs absent. Generate all reachable non-well-known descriptors
for both Proto and Descriptor sources. Keep protox validation, isolated descriptor-only
compiler input,32-file/path/flag/output/time/cancel budgets and explicit runtime/plugin
configuration. Exclude unused supplied files and built-in google/protobuf runtime outputs.

Write shared multi-file fixture and consumer before testing: imported enum/bytes/repeated/
oneof/Timestamp model plus service unary/server/client/bidi streaming. Eight builtin
language generation cases must include imported model/enum outputs; source and descriptor
outputs must preserve the reachable graph. Explicit grpcio-tools Python output must import
and run all four modes over loopback for both source representations. Preserve source and
plugin runtime metadata. Record baseline missing imports, apply smallest mature protoc
input fix, run actual fixtures and generation/API/lint/format gates, then commit/merge/push.
Other-language compile/call/plugins and deployment evidence remain full-scope work.

Ruling: classify built-in well-known imports using the same mature GoogleFileResolver,
not a google/protobuf namespace prefix — a supplied custom path under that prefix must
still emit its model. Add a direct existing workspace protox dependency to generation;
no new parser or descriptor registry. Custom-prefix generation is checked with C++;
per-language namespace/runtime packaging remains in broader compilation scope.

Verified: baseline C++ missing imported header and Python missing models module reproduced.
Eight explicit builtin protoc36.1 targets now emit imports with source/descriptor identical
code (16 cases), WKT runtime exclusion, custom-prefix retention and imported flag-path
rejection. grpcio-tools/protoc31.1 + grpcio1.84.0/protobuf6.33.6 consumers pass all four
modes from each input representation (8 RPCs).78 normal generation/11 API cases and
strict scoped Clippy/format plus parser tests pass; tool hashes/versions and runtime
metadata retained in isolated QA output. Other language compile/call/plugins remain open.
