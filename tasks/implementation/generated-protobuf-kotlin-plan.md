# Explicit Kotlin protobuf/gRPC Java service dependencies

Parent: milestone 2 / generated-protobuf-imports-plan.md / code-generation.md.

Official protoc-gen-grpc-kotlin coroutine stubs reference Java gRPC service descriptors;
current Kotlin generation emits Java messages but only invokes the Kotlin gRPC plugin.
Use hash-verified official Kotlin1.5.0 JAR with an explicit existing JDK21 launcher and
Java plugin1.84.1/protoc36.1. Extend trusted fixture selection with explicit Kotlin +
Java-plugin argument, preserving native/Python/Java/C# contracts. Prepare Gradle8.14.5 /
Kotlin2.4.0 separate consumers with protobuf Java/Kotlin4.36.1, gRPC Java1.84.1,
grpc-kotlin-stub1.5.0 and coroutines1.10.2; retain original SDK model/service output.

Write complete consumers/tests before running: source/descriptor compile and coroutine
unary/server/client/bidi loopback calls with bytes/Unicode/enum/repeated/oneof-zero/Timestamp,
original hashes/locks/tool/dependency evidence and cleanup. Reproduce absent Java service
output before repair; require explicit Java plugin when Kotlin gRPC is selected, invoke
both plugins in the same bounded protoc process, and preserve message-only/other targets.
Test missing-helper rejection without invoking tools, actual configured plugin outputs,
Python regression and generation/owned API/scoped Clippy/format gates. No copied custom
service descriptor/model codec or fallback. Document exact dependencies/platforms;
commit/merge/push verified batch. Other Kotlin libraries/ranges/platforms/schema/options/
TLS/auth and remaining plugins stay full-module work.

Verified: baseline actual Kotlin output omitted configured Java service helper. Both
plugins now run under the same process/budget; missing-helper guard rejects before tools.
Official Kotlin JAR SHA256 matched before execution; source/descriptor consumers compile
and pass all four coroutine modes (8 RPCs), preserving original hashes/locks and matching
code across representations. Recorded protoc36.1/Kotlin plugin1.5.0/Java plugin1.84.1,
Gradle8.14.5/Kotlin2.4.0/JDK21.0.12.1+1/protobuf4.36.1/grpc-kotlin1.5.0/coroutines1.10.2.
Python regression passed8 RPCs;5 configured plugin/builtin tests,79 normal generation
cases (38 ignored),11 owned API cases, parser tests and strict scoped Clippy/format pass.
Broader scopes above remain open; no copied descriptor/model/transport workaround used.
