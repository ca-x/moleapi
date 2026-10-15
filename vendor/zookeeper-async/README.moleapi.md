# ZooKeeper SDK source

This is the MIT-licensed `zookeeper-async` 5.0.0 crate from <https://github.com/krojew/rust-zookeeper>, copied from the checksum-verified Cargo registry source. The local copy caps inbound frames, decoded buffers and provider child count, rejects malformed UTF-8 without panicking, aborts SDK background tasks when a one-shot client is dropped, and omits request payloads from `Debug` output because digest authentication sends credentials in that payload. These changes keep untrusted ZooKeeper responses, secrets and cancellation bounded in the MoleAPI registry route. `LICENSE` is retained.

The remaining small edits are Clippy compatibility changes in the vendored crate. No protocol behavior is intended to change in those lines.
