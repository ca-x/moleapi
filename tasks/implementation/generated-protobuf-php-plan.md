# Explicit PHP protobuf/gRPC client four-mode calls

Parent: milestone 2 / generated-protobuf-imports-plan.md / code-generation.md.

Use official already-signature-verified grpc1.84.0 PHP protoc plugin and separately
PGP-verified php-grpc1.84.0 extension extracted only into QA prefix. Existing explicit
PHP8.5.11 NTS API20250925 matches; verify loading with isolated gRPC library path and
retain all package/signature/tool evidence. Official PHP docs use a non-PHP server.
Validate unchanged PHP generated clients against actual generated Python gRPC server,
with identical checked source/descriptor graph and four-mode wire observations.

Extend trusted php fixture selection and actual-plugin emission test. Write complete
consumer before tests: Composer grpc/grpc1.82.0/google/protobuf5.36.1 with no scripts/
plugins, generated model/service imports and synchronous/streaming unary/server/client/
bidi calls for both inputs. Preserve Unicode/bytes/enum/repeated/oneof-zero/Timestamp,
original hashes, locks/dependency/extension/runtime metadata and cleanup. No fake PHP
server, custom codec, client replacement or silent library fallback. Run Python default
regression and generation/owned API/scoped Clippy/format gates; record precise PHP-client
scope and commit/merge/push. PHP server/dependency/platform/TLS/auth/advanced schema/options
and remaining languages/plugins stay explicit full-module work.

Ruling: PHP package release numbering differs from extension and other-language runtimes.
Composer availability and official Packagist metadata confirm grpc/grpc1.82.0 and
PHP google/protobuf5.36.1, with native extension/plugin1.84.0/protoc36.1. Pin actual
published PHP dependencies explicitly; no alternative library or transport is substituted.

Verified: signed native extension/plugin1.84.0 loads in selected PHP8.5.11 NTS API20250925;
resolved Composer grpc/grpc1.82.0 and PHP Google/protobuf5.36.1 retain source refs/locks.
Both PHP source/descriptor clients syntax-check/import and pass all four modes against
actual generated Python grpcio1.84.0 servers (8 RPCs), with identical descriptors and
unchanged matching generated hashes. Actual-plugin emission test passes; Python default
regression passes8 RPCs.79 generation cases (41 ignored),11 owned API cases, parser tests
and strict scoped Clippy/format pass. No emitter/model/transport repair was needed.
All wider PHP server/range/platform/schema/option scopes above remain open.
