# mysql_async 0.37.1 transport disposal patch

Upstream: crates.io mysql_async 0.37.1. Original MIT/Apache licenses retained.

Adds an opt-in `immediate-drop` feature: dropping a direct, non-pooled connection
synchronously drops its transport instead of spawning unbounded result-drain and
rollback cleanup. Pooled connections retain upstream behavior. Explicit
`disconnect()` remains the graceful SDK path. Adds `Conn::abort()` to dispose
pending result transports without protocol parsing or draining.

MoleAPI enables the feature so canceled handshakes, cancellation connections,
timeouts and session disposal cannot spawn untracked cleanup. No protocol codec,
authentication or SQL parser is replaced. Abrupt closure cannot prove a write did
not commit; cancellation retains the existing uncertain-write feedback. Capped
queries retain captured rows, dispose their connection and require reconnecting.
