# Explicit Java protobuf/gRPC four-mode consumers

Parent: milestone 2 / generated-protobuf-imports-plan.md / code-generation.md.

Use the documented official protoc-gen-grpc-java1.84.1 Linux executable with explicit
protoc36.1 and existing isolated JDK21/Maven3.9.11. Verify the artifact checksum and record
source/tool metadata. Extend trusted fixture generation to Java without changing normal
Python invocation or product runtime configuration. Keep original emitted imports,
messages/service interfaces and descriptor/source manifests; build a separate Maven
consumer with matching protobuf-java4.36.1 and gRPC1.84.1 dependencies. Dependencies stay
in isolated QA repositories; product preview does not install, compile or call them.

Write all consumers/tests before running them: Java model compile and actual loopback
unary/server/client/bidi calls for both source/descriptor inputs. Preserve Unicode,
non-UTF8 bytes, enum/repeated fields, oneof presence including code=0 and Timestamp.
Add an explicitly configured actual-plugin emission test and preserve Python fixture
regression. Repair only reproduced engine/runtime integration defects; keep plugin/path/
source/output budgets and cancellation. Record actual versions and scope; run generation/
owned API/scoped Clippy/format checks, then commit/merge/push verified batch. Other plugins,
platforms, TLS/auth/advanced/dependency options and full scope remain open.

Verified: published official plugin SHA256 matched before execution. Both source and
descriptor Java consumers compile/call all four modes over Netty (8 RPCs), using
protoc36.1/plugin1.84.1/gRPC1.84.1/protobuf-java4.36.1/Temurin21.0.12.1+1/Maven3.9.11.
Generated hashes match across representations and remain unchanged after build/runtime;
dependency tree/tool/descriptor metadata retained with strict checksums and isolated repo.
Actual-plugin emission test passes; Python default fixture regression passes8 RPCs.
78 generation cases (36 ignored),11 owned API cases, parser tests and strict scoped
Clippy/format pass. No transport/model/plugin implementation patch was needed.
