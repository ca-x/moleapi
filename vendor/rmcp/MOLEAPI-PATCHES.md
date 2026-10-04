# Official rmcp 3.5.0, bounded transport patch

Source: crates.io rmcp 3.5.0 (official modelcontextprotocol/rust-sdk), Apache-2.0.
All SDK source is retained; MoleAPI's changes are restricted to transport input limits:

- `src/transport/async_rw.rs`: cap each buffered STDIO line at 1 MiB before reading, and total raw input at 8 MiB. EOF/limit closes the SDK transport. The SDK still owns all framing/deserialization and cancellation.
- `src/transport/common/reqwest/streamable_http_client.rs`: consume JSON and error HTTP response bodies with a streaming 1 MiB cap before deserialization/body previews. SDK SSE already enforces a raw event limit; MoleAPI config sets it to 1 MiB.

- `src/transport/common/client_side_sse.rs`: cap each raw SSE stream at 8 MiB including comments/keepalives, in addition to the SDK event-size limiter.
- `src/transport/child_process.rs`: on explicit transport close, terminate the owned process group/job and await the SDK wrapper's reaper; do not allow a gracefully exiting leader to leave descendants alive. Drop synchronously signals the group/job before scheduling the SDK reaper, so runtime shutdown cannot strand a detached process tree.

No protocol model, negotiation or parser behavior is replaced. Reevaluate against upstream when upgrading.
