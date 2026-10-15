# Explicit Rust request-snippet consumer QA

Cargo.toml/Cargo.lock pin the validation consumer, not MoleAPI's Rust dependencies.
The driver copies them to a fresh temporary project and supplies src/main.rs from
unchanged trusted fixture snippets inside async functions, followed by response
assertions. A matching Rust toolchain and Node HTTP fixture runtime are required.

Current evidence: Rust1.99.0/Linux, Reqwest0.13.5, Tokio1.53.3, edition2024. Reqwest's
form/multipart/rustls and Tokio's macros/rt-multi-thread features are explicit.
The generated request snippets do not need serde_json for raw JSON bodies.
Other versions/platforms need their own native checks; no compiler/runtime is
installed, downloaded or invoked by MoleAPI when a user generates an example.

```sh
python3 tools/snippets/verify-rust.py --node /absolute/node --output /absolute/fresh-output
```

Cargo/rustc are selected by the developer's toolchain environment and recorded
with versions and actual binary hashes. An explicit CARGO_TARGET_DIR is honored
through Cargo metadata. The app fixture runs in the repository; the isolated
consumer uses its own lock and native HTTP client. Runtime proxy overrides are
removed for the owned loopback fixture. Cargo dependency resolution/builds retain
the developer's build environment. Original saved request/snippet/engine/lock/tool/
consumer hashes, resolved dependency tree and observed wire bytes are preserved.

The21 profiles cover raw files/MIME/GET/HEAD, raw and JSON/control/Bidi literals,
repeated/empty forms, multipart metadata/duplicates/text-only forms/disabled parts,
headers/auth/query and source immutability. Compilation and all calls are required;
missing tools or failed native cases are not skipped. File reads use std::fs and
materialize selected bytes in the consumer; Reqwest owns MIME framing and boundary.
