# Original Objective-C native QA consumer

The verifier runs on actual Apple Foundation/Xcode hosts and calls a real generated
Python gRPC server. Original GPB/gRPC Objective-C files are referenced in place,
with byte hashes checked before/after. Both source/descriptor representations and
all unary/server/client/bidi calls remain required. Generated GPB messages retain
MRC, service/QA code ARC, and the Swift linkage unit required by pinned Protobuf.

The successful arm64 evidence is run38707843936 at immutable62f08e8: both RPC
representations and both3.0.3/3.1.0 model dialects passed on macOS26.6.2/Xcode26.6.
Intel retry38718238486 passed source RPCs but its descriptor Release build timed
out after3600sec. Logs showed3088 arm64 and3089 x86_64 CompileC lines across the
separate consumers. That is build cost evidence, not a protocol/model assertion
failure or a reason to omit a representation.

## Sequential shared consumer

Pass the same absolute --consumer-dir to both verifier calls on one host. The
private directory bootstraps the pinned gRPC1.84.0/Protobuf5.36.1 Pods once. Its
marker binds schema descriptor, scripts, actual CPU/Xcode, Podfile and installed
lock/project hashes. Foreign/incomplete/drifted directories are refused without
automatic deletion or repair. Select a fresh directory when inputs change; shared
consumers are sequential, not concurrent.

The Ruby updater retains the project/target/Generated-group IDs, runtime/Swift
files, CocoaPods integration and base configurations. Only original generated-file
references and header roots switch. The second representation uses the same Pods
and derived data, avoiding a second cold gRPC-Core compilation. Release and the
3600sec build bound remain unchanged. Explicit macOS destination/ARCHS and
ONLY_ACTIVE_ARCH select only the executing host; lipo validates the binary before
actual RPC execution. Cross-architecture execution belongs to its separate job.
Evidence additionally records build command, binary architecture/hash, dependency
project hash and whether the consumer was reused.

Local tests use the real xcodeproj1.27 parser/writer and a lock-shaped dependency
fixture to verify source switching, identities, MRC flags, immutability and cache
refusal. They do not compile/execute Apple code or establish native RPC success:

```sh
python3 tools/projects/test_objc_consumer.py --ruby /absolute/ruby
```

Use the committed Gemfile/lock (or equivalent exact xcodeproj dependencies) in an
isolated Ruby environment. Native acceptance still requires the new immutable
Apple workflow run and its two actual representation evidence files.
