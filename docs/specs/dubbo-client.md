# Classic Dubbo client

Matrix row `protocols-015`; parent plan milestone 3. This specification covers Apifox-compatible classic Dubbo2 TCP transport with Hessian2 serialization. Dubbo Triple uses HTTP/2 and protobuf and is a separate protocol option; existing gRPC calls cannot be presented as classic Dubbo support.

## User workflow

An owner creates a Dubbo request, selects a direct `dubbo://host:port` endpoint or an explicitly configured registry, enters a Java service interface, method, optional version/group and ordered typed arguments, then invokes it. JSON arguments include explicit Java class wrappers (`$class`/`$`) for ambiguous DTOs, lists and long integers. The workbench shows the decoded value or remote exception, timing and bounded diagnostics; scripts, assertions and saved source use the existing request lifecycle where supported.

The first executable slice is direct classic unary invocation with fixed endpoint, explicit service/method/version/group, ordered arguments and attachments. ZooKeeper/Nacos/EDAS provider discovery and metadata import are separate required slices. The source model must keep those future fields distinguishable and reject selected unsupported registry settings until implemented.

## Source and runtime contract

- Save the original source and parameter order; variables resolve into an execution copy. Native backup keeps original credentials and class names; default exports/history screen known private values. Unmapped Postman/OpenAPI exports reject rather than relabel the request as HTTP or gRPC.
- Use an independently interoperable Dubbo2/Hessian2 library for framing and values. The application supplies owner admission, checked and pinned destination sockets, deadlines, cancellation, budget checks and result projection. The library must accept an injected socket or receive a narrowly maintained connector patch; no second unchecked resolver/connect path.
- Direct destinations pass the same hosted private-network policy, DNS pinning and TLS/proxy compatibility rules as raw TCP. A selected transport setting that the library cannot honor returns an explicit error. Registry and metadata endpoints independently pass network checks.
- Bound source, argument count/depth/bytes, attachments, request/response frames, connection time, total request time, and decoded output before allocation or history storage. A peer-declared length cannot trigger unbounded allocation. Stop closes the owned connection and does not imply remote rollback.
- Preserve remote status versus transport failures. Never treat a null result, Hessian exception or incomplete frame as a successful empty response. Limit response disclosure using the current privacy/redaction pipeline.
- The editor uses bilingual labels, keyboard controls, clear validation, 390px layout and the existing workbench panels. It does not expose a false choice for unimplemented Triple or registry modes.

## Library finding and gate

The recorded Apache `dubbo 0.4.0` Rust crate implements Triple only. `dubbo-rs-protocol-dubbo 0.1.0` claims classic TCP/Hessian2, but its current `DubboClient` calls `TcpStream::connect` internally and `read_frame` allocates a peer-declared body length without an application bound. Its request body writes arguments as Hessian binary items. These findings require an actual Apache Dubbo Java provider interoperability fixture and a checked-socket/bounded-frame integration design before adopting it. Its README claim is not acceptance evidence. A pinned official Java Dubbo client running in an isolated, bundled-JRE worker is a candidate fallback if the Rust package cannot preserve correct wire semantics without replacing its core.

## Acceptance evidence

1. Independent Java provider receives scalar, signed/unsigned boundary, string, DTO, list and multi-argument requests with exact types, version/group and attachments; returns value, null and exception. Original Hessian bytes/decoded results and source remain inspectable.
2. Real direct connection proves checked DNS, hosted private-IP refusal, timeout, cancellation, reconnect and malformed/oversized peer frames without an unchecked route or large allocation.
3. Hosted owner/workspace isolation, source roundtrip/default-private/explicit export, script/assertion/history boundaries, and actual React workbench behavior pass focused tests. Unsupported registry/transport/export options fail visibly.
4. Each later registry/metadata provider has separate real service discovery and auth/privacy/ownership evidence. No source or catalog claims full Dubbo parity after only direct unary execution.

Apifox source evidence is the retained 2026-10-02 complete, hash-checked content for `create-dubbo-api.md` and `debug-dubbo-api.md` in `docs/references/raw/`: it explicitly names classic Dubbo transport plus Hessian, manual Package/Service/Method definitions, direct/registry calls and JSON typed parameters. Full release still requires the matrix-wide gates.
