# Objective-C gRPC generated interfaces and Apple validation

Parent: milestone 2 / code-generation.md / PROJECT-GENERATION-COVERAGE.md.

Use official already-signature-verified grpc1.84.0 objective-C protoc plugin for exact
source/descriptor artifacts on Linux, preserving original Foundation/GPB/GRPC interfaces.
Apple runtime requires official gRPC Objective-C macOS11+/iOS15+ APIs; GNUstep or source
text emission cannot stand in for Apple compile/call evidence. Add trusted objc fixture
selection and actual-plugin emission/regeneration tests, inspecting real generated naming
and stream APIs. Prepare Apple-only consumer/CI acceptance from mature CocoaPods/protobuf/
gRPC libraries and actual generated Python server interfaces, covering all four client
modes, Unicode/bytes/enum/repeated/oneof-zero/Timestamp and cleanup.

Inspect configured macOS workflows/runners and retain exact current-head job/artifact
handles. Run local implementation/test gates after consumers/tests are written; dispatch
actual macOS checks once concrete CI code is reviewable. Queue waits do not establish
success. Preserve full Objective-C runtime/platform scope until actual Apple evidence;
other functional generation slices remain independent work if runner state blocks it.

## 2026-10-15 implementation checkpoint

- Added trusted `objc` fixture selector and real official-plugin regression test.
- Original source/descriptor `.h`/`.m` artifacts are identical; the plugin test passes
  with protoc36.1 and the signature-verified gRPC1.84.0 Objective-C plugin on Linux.
- Added Apple-only v2 consumer for all four RPC modes, with Unicode, bytes, enum,
  repeated fields, oneof zero presence and Timestamp equality. Uses mutable call
  options and retains response queues/messages under ARC; original generated GPB
  model implementations compile with manual reference counting.
- Added Xcode project builder, locked CocoaPods Ruby tooling, and a native-runtime
  driver using gRPC-ProtoRPC1.84.0 / Protobuf5.36.1 against generated Python services.
  The driver compares server/client descriptors, checks observed requests, stops
  the server, preserves source hashes, and records Xcode/platform/Podfile.lock.
- Existing build workflow accepts `objc_qa=true` on dispatch, covering Intel/ARM
  macOS without launching unrelated server/desktop jobs. QA-only Homebrew tools
  are explicit and their actual versions/hashes are retained; these are not
  application-managed installations.
- Local gates: 80 regular generation cases passed, 43 external-tool cases ignored;
  actual Objective-C plugin case passed; scoped Rust format, Python compile, Ruby
  syntax and actual xcodeproj project construction passed. Linux runtime rejection
  confirmed; these checks do not establish an Apple build or RPC success.
- Full workspace formatting hits an existing vendored mustache workspace-layout
  error; package-scoped generation formatting passes.
- Apple compilation and runtime evidence remain pending actual macOS jobs.

Apple dispatch: run38637719998 validates d0f059a (Intel job116105064831 and
ARM job116105065093); both queued at the last check. Official Protobuf5.36.1
podspec requires macOS12.0, despite gRPC's macOS11 minimum. Set the QA target
and Podfile to12.0 for this pinned combination; macOS11 runtime support remains
separate unverified scope. Queue status does not establish runtime success.

Run38639078923 on df02ba9 reached both real macOS workers and failed in Python
dependency installation: grpcio-tools1.84 requires protobuf>=7.35.1,<8, incompatible
with pinned6.33.6. No Objective-C build or call ran. The actual previously verified
Linux fixture environment uses grpcio1.84.0 / grpcio-tools1.76.0 / protobuf6.33.6;
1.76 requires protobuf>=6.31.1,<7 and grpcio>=1.76. Update only that compiler pin,
record Python freeze with tool evidence, and resolve macOS Intel/ARM wheels before
redispatch. Keep native Apple build/call acceptance open until successful jobs.

The corrected exact Python trio resolved and downloaded universal2 wheels for both
macOS11 ARM and x86_64 / CPython3.13 without conflicts. Actionlint passes with the
existing Intel runner label declared to the locally older checker. This establishes
package availability and dependency compatibility, not an Apple build/RPC success.


Run38641146152 on94eda9c: actual Intel Mac emitted both Objective-C source/descriptor
artifacts, then failed generating Python server fixtures. Exact finished job116116660193
logs prove this boundary; the ARM job is independently still building. Application
protoc execution intentionally env_clear()s both generation and version queries. The
CI shell launcher depended on ambient python/PATH, unlike the previously verified
absolute Python entrypoint. Cleared-env local reproduction fails with the shell
launcher and passes libprotoc31.1 with the exact selected absolute Python shebang.
New fixture source/descriptor generation and8 actual Python regression RPCs pass.

CI now persists the selected Python executable via task-specific GITHUB_ENV and uses
it for the native driver too; startup imports grpc_tools to fail before expensive
builds if the selected interpreter is wrong. QA Rust cache saves completed compile
work even when a later acceptance phase fails, preventing repeated cold-build cost.
Actionlint passes. This fixes QA interpreter selection, not an Apple runtime claim.
Real Apple compile/call acceptance remains pending corrected job execution.
