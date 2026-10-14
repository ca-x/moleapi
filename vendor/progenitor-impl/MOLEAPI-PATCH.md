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
incompatible declared/raw-success pagination contracts remain rejected. This emitter patch is used by the generator,
not required by generated projects. Generated CLI projects depend on tokio-tungstenite
for frame handling; they do not depend on this vendored implementation.


Raw-body metadata now retains the original requestBody.required flag. Optional raw
positional/builder methods preserve None and attach a Content-Type/body only for Some;
required raw behavior and typed JSON/form bodies are unchanged. CLI --body-file follows
requiredness, uses read_to_string for text and read for octet streams. Generated CLI and
positional SDK projects passed actual omitted/empty/provided binary/text wire cases;
existing raw/upgrade consumers passed. Buffered upload remains an explicit limitation.


For recognized typed Dropshot pages, raw error branches retain the SDK error/nonzero
exit instead of todo. The CLI limit uses the exact schema-derived Clap type and positive
integer conversion, fixing an actual NonZeroU64/NonZeroU32 downcast panic. Unsupported
explicit pagination contracts reject before CLI rendering; no alternate page parser is
introduced. Two standalone typed-page limit variants passed real multi-page, limit and
first/later raw-error wire fixtures while preserving upstream lazy token streaming.
