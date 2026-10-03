# Official Socket.IO fixture

`server.cjs` uses the official `socket.io` server package, pinned to 4.8.1 with an integrity-locked dependency tree. Socket.IO protocol revision 5 / Engine.IO revision 4, WebSocket transport only. It listens on loopback; no Docker.

From this directory:

```sh
npm ci
node server.cjs
```

The fixture listens on port 18886 by default (`PORT` overrides it). Use namespace `/fixture`, Engine.IO path `/custom/socket.io/`, auth JSON `{"token":"fixture-token"}`, header `X-Fixture: yes` and query `required=yes`.

From the repository root, with the fixture running:

```sh
cargo test -p moleapi-protocols --lib --test socketio -- --include-ignored
cargo test -p moleapi-server --no-default-features --test socketio -- --include-ignored
```

The tests that require Node are explicitly ignored during the default Rust suite so it stays offline and requires no npm installation. Unconditional Rust tests cover config, interpolation, drafts, quotas, invalid arguments, ACK expiry/admission, private-network denial, handshake deadline and pending cancellation. The explicit integration run includes all Node cases; the TLS case starts and stops its own Node HTTPS server on an ephemeral loopback port, checks original-hostname SNI, rejects the self-signed certificate with verification enabled, and exercises the explicit verification override. `NODE_PATH` can point to an isolated installation's `node_modules` instead of installing dependencies into this directory. `MOLEAPI_SOCKETIO_FIXTURE_URL` overrides the protocol integration tests' plain fixture URL.

Events:

- `echo`, `error`, `open`, `close`: echo all positional JSON/binary arguments and acknowledge with identical arguments. The latter three are ordinary application event names; they must not trigger client lifecycle changes.
- `ack-error`: ACK success with the application's `{error:"application-error",code:42}` argument. Socket.IO itself has no universal application-error ACK convention.
- `ack-timeout`: deliberately never acknowledges.
- `server-ack`: emits `question` with JSON and bytes plus a server callback; `server-ack-result` reports the client reply.
- `private`: echoes a value and the received auth token to verify redaction.
- `close-now`: server namespace disconnect.
- `burst`: emits 300 events to exercise cursor retention.
