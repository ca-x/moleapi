# Explicit C++ protobuf/gRPC four-mode consumers

Parent: milestone 2 / generated-protobuf-imports-plan.md / code-generation.md.

Host has GCC/protobuf36.1 development dependencies but lacks gRPC/RE2/CMake. Use official
Arch grpc1.84.0-1 and RE2 packages extracted only into QA prefix; verify detached PGP
signatures against existing trusted Arch keyring (dearmor its public representation
for gpgv). No system package installation. Explicit plugin launcher selects the verified
ELF/library path under the product's cleared process environment. Existing host protobuf/
abseil/c-ares/OpenSSL/zlib are recorded. Compile consumers directly with explicit G++/
pkg-config plus isolated prefix, avoiding an unnecessary CMake distribution dependency.

Extend trusted cpp fixture selection and actual-plugin generation test. Write complete
C++ consumers before tests: unchanged generated models/service interfaces compile and
actual synchronous loopback unary/server/client/bidi calls across source/descriptor.
Preserve Unicode/non-UTF8 bytes/enum/repeated/oneof-zero/Timestamp and clean shutdown.
Record SDK hashes, compiler/package/link metadata and runtime evidence. Retain mature
protobuf message comparison and original gRPC transport; no model/service/codec mock.
Run Python regression and generation/owned API/scoped Clippy/format gates; document exact
versions/platform and commit/merge/push. Other compiler/platform/library ranges, advanced
schema/options/TLS/auth and remaining language plugins stay full-module work.

Verified: official grpc/RE2 PGP signatures matched existing trusted Arch keyring. Isolated
prefix/explicit ELF launcher links verified libraries; host packages unchanged. Original
source/descriptor models/service files compile and pass four modes (8 RPCs), retaining
source hashes/matching code and actual build/link maps. GCC16.2.1/C++17/grpc1.84.0/
protobuf36.1.0/RE2 2025.11.05/GNU Linux; host dependencies recorded in coverage.
Actual-plugin emission test passes; Python regression passes8 RPCs.79 generation cases
(40 ignored),11 owned API cases, parser tests and strict scoped Clippy/format pass.
No model/transport/emitter patch was needed; wider scopes above remain open.
