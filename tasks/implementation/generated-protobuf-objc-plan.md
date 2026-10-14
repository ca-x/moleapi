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
