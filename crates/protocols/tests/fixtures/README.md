# Real gRPC fixture

`service.proto` imports `types.proto`; generated `moleapi.fixture.rs` and `schema.bin` were produced with tonic-prost-build 0.14.6/prost-build 0.14.4 and protoc. They are test assets only; production schema import uses protox and does not require protoc or filesystem access.

Regenerate from a temporary Cargo binary with `tonic-prost-build = "0.14.6"` and this main function (run from this directory):

```rust
fn main() {
    tonic_prost_build::configure()
        .out_dir(".")
        .file_descriptor_set_path("schema.bin")
        .compile_protos(&["service.proto"], &["."])
        .unwrap();
}
```

Run the manual target from the repository root:

```sh
cargo run -p moleapi-protocols --example grpc-fixture -- 127.0.0.1:18884
```

Reflection v1 and v1alpha expose `moleapi.fixture.EchoService`: `Unary`, `ServerStream`, `ClientStream`, `Bidi`. Unary `{"text":"error"}` returns PermissionDenied with actual binary details/trailer metadata. Unary `{"text":"wait"}` waits20seconds; ServerStream waits after its first message. `x-require-auth: yes` requires Bearer `fixture-secret`; `x-require-auth: basic` requires Basic `user:pass`. These credentials are deliberately public test values.
