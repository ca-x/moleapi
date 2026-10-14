# Generated native CLI upgraded connections

Parent: docs/specs/code-generation.md and remaining-work-plan milestone2.

Contract: preserve Progenitor operation classification, arguments and SDK upgrade handshake.
CLI dispatches successful upgraded ResponseValue through a new async output hook. Generic
101 transports stream optional --upgrade-input file bytes to the connection, half-close
outbound at EOF and stream inbound bytes unchanged to stdout or create-new --output-file.
Declared x-dropshot-websocket endpoints use tokio-tungstenite over the SDK upgraded transport;
optional input file is one binary message, incoming text/binary message payloads stream
unchanged until server close, with library control-frame handling. These are explicit file
input contracts; interactive terminal/framing/subprotocol workflows remain separate scope.
Errors retain the SDK error/nonzero exit; paginated raw responses stay guarded.

1. Add upstream CLI hook and dispatch, adapter and dependency; preserve MPL notices.
2. Write generated standalone compile/process fixtures covering generic duplex bytes,
WebSocket masked binary input/text+binary output/ping/close, create-new output preservation
and typed HTTP400/failed101 handling. Add fast artifact/source regressions. Complete code
and test consumers before executing the batch.
3. Run meaningful generated build/process tests, generation/API regressions, scoped lint
and formatting. Diagnose exact failures, document evidence/limits, commit/merge/push.


Implemented/verified: generated standalone CLI passed6 upgrade requests and5 raw binary
regression requests;2 targeted artifact/process cases,67 generation regressions and11
owned-worker/privacy API cases passed. Precise remaining transports/interactive/framing
and handshake coverage stay open in the parent coverage document.
