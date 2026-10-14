# Progenitor implementation patch

Source: crates.io progenitor-impl 0.15.0, https://github.com/oxidecomputer/progenitor.
Upstream license: MPL-2.0; existing file copyright notices retained.

The CLI emitter is extended (with crate-visible existing WebSocket metadata): finite raw success responses call an asynchronous
`CliConfig::success_raw` hook, and raw error responses return the original typed
SDK error instead of an emitted panic. Raw request bodies get a --body-file
argument consumed by the upstream SDK builder. All operation discovery, arguments, request
construction and response classification remain upstream. Upgrade responses call an async
`CliConfig::success_upgrade` hook with the existing WebSocket classification. Upgrade
errors retain the SDK error. The executable adapter uses Tokio duplex IO for generic
upgrades and tokio-tungstenite over the SDK upgraded connection for declared WebSockets;
raw paginated responses remain guarded. This emitter patch is used by the generator,
not required by generated projects. Generated CLI projects depend on tokio-tungstenite
for frame handling; they do not depend on this vendored implementation.
