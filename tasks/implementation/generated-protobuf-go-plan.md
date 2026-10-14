# Explicit Go protobuf message/gRPC plugins and consumers

Parent: milestone 2 / code-generation.md / PROJECT-GENERATION-COVERAGE.md.

Add protobuf-go through official protoc-gen-go (required messages) and optional official
protoc-gen-go-grpc (service interfaces). Keep roles separate: local/admin-only explicit
MOLEAPI_CODEGEN_PROTO_GO and MOLEAPI_CODEGEN_GRPC_GO paths, no PATH discovery/download or
imported option paths. Preserve existing with_protoc API and other targets. Add bounded
message-plugin builder/state, catalog dependency keys and Go target. Missing required
message plugin rejects before tools; messages without gRPC remain supported. Mature
plugins own Go naming/package/path behavior: preserve authored go_package and use normal
import-path output, never rewrite user files or invent source-relative broken packages.

Use a separate trusted Go fixture derived from the shared multi-file schema with authored
go_package options. Generate source/descriptor via explicit pinned plugins built with
Go checksum-db verification in isolated GOPATH/module caches. Write unchanged-code local
module consumers and actual unary/server/client/bidi -race loopback tests, preserving
enum/repeated/bytes/Unicode/oneof-zero/Timestamp and joined server cleanup. Record Go module
locks/source hashes/tool/plugin/dependency/descriptor evidence, plus message-only generation.

API exposes configured message plugin names without paths. Apply ui-ux-pro-max and
emil-design-eng: adjacent bilingual dependency explanations and disabled Generate state
for missing Go message plugin; fix existing Kotlin missing-Java helper notice using the
same dependency rules. No new animation for repeated/keyboard actions. Test real UI
selection, blocked submissions, restored ready state and locale changes; rendered browser
checks for both locales at desktop/mobile. Preserve owner/open/result fences. Follow modern
Go CLI1.27 guidance, meaningful Rust/API/frontend gates after code/tests are complete.
Commit/merge/push verified slice; other SDK/plugin/platform/schema/TLS/auth and full
395-feature scopes remain open.

Ruling: embedded Rust server compilation overlapped Vite build, changing declared web
asset build-script inputs and MBX discarded that compilation. Finish web build first,
then build embedded server sequentially; retain the input-change verification guard.

Verified: message plugin required/path roles explicit; Go service plugin optional, authored
import paths preserved and missing go_package rejects. Original source/descriptor outputs
compile and pass four -race modes (8 RPCs), hashes equal/unchanged; sums/locks/build/tool
metadata retained for Go1.27.1/protobuf1.36.12/plugin-go-grpc1.6.2/grpc-go1.84/protoc36.1.
Required/optional/reopened/locale/UI state tests and actual embedded browser English/Chinese
1280px/390px checks pass with visible blockers/no overflow; Protobuf description/disabled
cue repaired.29 UI/i18n tests/frontend build,80 generation cases (42 ignored),12 API cases
and scoped strict Clippy/format pass, plus actual Go plugin/missing-source/messages-only
and all8 builtin source/descriptor regression checks. Native/hosted catalog share names
without executable path disclosure. Broader scopes above stay open.
