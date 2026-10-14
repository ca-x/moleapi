# Explicit C# protobuf/gRPC four-mode consumers

Parent: milestone 2 / generated-protobuf-imports-plan.md / code-generation.md.

Use official Grpc.Tools2.84.0 Linux plugin with explicit protoc36.1 and existing isolated
.NET10.0.401 SDK. Verify NuGet package signatures before extracting the one selected plugin;
record URL/hash/source evidence. Extend trusted generation with explicit csharp selector
without changing native Rust/Python/Java contracts. Keep original generated models/service
interfaces/descriptor/source manifest; no runtime/library fallback or copied custom codec.

Write separate .NET consumer before tests: original model/service compile and actual
Kestrel HTTP2 + Grpc.Net.Client unary/server/client/bidi calls for both source/descriptor
inputs. Preserve enum/repeated/Unicode/bytes/oneof zero presence/Timestamp and cleanup.
Use matching Google.Protobuf3.36.1 and gRPC.NET2.84.0; isolate package/CLI-home directories,
retain restore lock/assets/dependency/tool evidence. Add actual-plugin generation test,
keep Python regression and run generation/owned API/scoped Clippy/format gates. Record
exact support, then commit/merge/push. Other libraries/runtimes/platforms, TLS/auth/advanced
options/schema/budgets and remaining plugins remain full-scope requirements.

Ruling: NuGet leaf/catalog SHA512 endpoints returned404; the existing .NET SDK's
standard nuget verify --all successfully validated package signatures instead. Record
package SHA512 and extracted plugin SHA256 as observed hashes, not published-hash
comparisons. No trust bypass or certificate policy change is used.

Verified: NuGet repository signature passed before plugin extraction/execution. .NET10.0.401
release source/descriptor consumers compile and pass all four HTTP2/Kestrel modes (8 RPCs),
using protoc36.1/Grpc.Tools2.84.0/Google.Protobuf3.36.1/gRPC.NET2.84.0/Linux. Original code
hashes match and remain unchanged; locks/assets/dependency/tool/source/signature metadata
retained. Actual-plugin emission test passes; Python regression passes8 RPCs.78 generation
cases (37 ignored),11 owned API cases, parser tests and strict scoped Clippy/format pass.
No model/transport/emitter patch was needed; wider scopes above remain open.
