# Explicit Ruby protobuf/gRPC four-mode consumers

Parent: milestone 2 / generated-protobuf-imports-plan.md / code-generation.md.

Host Ruby is absent. Prepare isolated Ruby3.4.7 from official checksum-verified source
and gRPC1.84.0 / google-protobuf4.36.1 prebuilt GNU Linux gems using published RubyGems
SHA256 metadata, plus matching required gems. Verify grpc-tools1.84.0 package before
extracting its official Ruby protoc plugin. No system installation or product runtime
acquisition; explicit plugin/compiler/runtime/gem-home paths only.

Extend trusted fixture ruby selection and actual-plugin emission test; write complete
consumer before tests. Keep original generated model/service modules and compile/import
both source/descriptor outputs. Actual loopback unary/server/client/bidi calls preserve
Unicode/non-UTF8 bytes/enum/repeated/oneof-zero/Timestamp and stop server/threads. Record
original hashes, package metadata/tool versions and descriptor/runtime evidence; retain
required dependency versions without source edits/custom codecs/transport mocks. Run
Python regression and generation/owned API/scoped Clippy/format gates; document exact
platform/ranges and commit/merge/push verified batch. Other Ruby/plugin/runtime/library/
platform/options/schema/auth/TLS behavior and remaining plugins stay full-module work.

Verified: official Ruby3.4.7 tar.xz/public SHA256 matched and runtime built only in /tmp;
all selected grpc-tools/runtime/Rake package SHA256s matched RubyGems metadata. grpc-tools/
grpc1.84.0/protoc36.1/google-protobuf4.36.1/googleapis-common-protos-types1.23.0/rake13.4.2/
bundled bigdecimal3.1.8 on GNU Linux. Both source/descriptor modules syntax-check/import
and pass four modes (8 RPCs), preserving original hashes and full stream/server cleanup.
Actual Ruby plugin generation test passes; Python regression passes8 RPCs.79 generation
cases (39 ignored),11 owned API cases, parser tests and strict scoped Clippy/format pass.
No model/transport/emitter repair needed; broader scopes above remain open.
