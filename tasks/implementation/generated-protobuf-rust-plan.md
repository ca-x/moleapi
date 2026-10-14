# Native Rust protobuf/gRPC four-mode consumers

Parent: milestone 2 / generated-protobuf-imports-plan.md / code-generation.md.

Reuse the checked multi-file fixture (enum/bytes/repeated/oneof/Timestamp and four RPC
modes), native protox/tonic-prost-build emitters and capped application worker. Extend
the trusted fixture tool with an explicit rust-tonic selector requiring no protoc/JRE/
plugin; preserve Python/Java invocation contracts. Upgrade native source/descriptor
emission regression to include all four service interfaces and equivalent code.

Write a separate Rust SDK-path consumer before running builds. Keep generated Cargo.toml,
SDK source/descriptor bytes intact and record resolved consumer Cargo.lock/tool metadata.
Compile and call actual Tonic loopback unary/server/client/bidi transports for both inputs,
preserving Unicode/non-UTF8 bytes/enum/repeated/oneof zero presence/Timestamp and cleanup.
Run Python default fixture regression and generation/owned API/scoped Clippy/format gates;
record exact package/tool/platform evidence, commit/merge/push. Broader Rust schema/options/
auth/TLS/cancellation/SDK ranges/platform and remaining non-Rust plugins stay open.

Verified: native source/descriptor consumers compile and pass all four modes (8 RPCs),
preserving original SDK manifest/source hashes and identical generated code across inputs.
Consumer locks/dependency tree/tool/descriptor evidence retained. Cargo1.99.0/Linux,
Tonic/tonic-prost0.14.6, Prost/prost-types0.14.4, Tokio1.53.3/tokio-stream0.1.19.
Python default regression passed8 RPCs;78 generation cases (36 ignored),11 owned API
cases, parser tests and strict scoped Clippy/format pass. No native emitter/model/transport
patch was necessary; all broader scopes above remain open.
