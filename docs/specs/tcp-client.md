# TCP client

Authority: protocols TCP capability in FEATURE-MATRIX/PROTOCOL-COVERAGE, Apifox official tcp-socket.md research, user requires mature Rust crates and independent hosted/native execution. This is a dedicated TCP client, not a renamed HTTP request.

## Objective and contract

Portable Protocol `{kind:"tcp",framing:"raw"|"lines"|"length_be"|"length_le",max_frame_bytes:1048576,no_delay:true,idle_timeout_ms:0,message:{encoding:"text"|"hex"|"base64",payload_source:"",secret:false}}`. Store original editable payload draft; no automatic send on connect. Exact tcp:// or tcps:// URL with explicit nonzero port, no credentials/path/query/fragment. Shared request GET/None is a connection envelope; HTTP headers/query/auth and post-response scripts are explicitly unsupported at invocation. Scoped transport variables/pre-scripts remain supported. Payload variables resolve only when explicitly sent through the existing bounded literal interpolation helper against the connection’s captured scoped environment. Changing environment values requires reconnect.

Existing owner-scoped sessions API: `tcp_send {message:TcpMessage}` and `tcp_half_close`. Events `tcp_data {base64,text?:string|null,bytes,redacted}` with incoming/outgoing direction and `tcp_half_closed`. Read counters count actual wire bytes including framing; outgoing counter counts actual successful writes. Half-close shuts down only the writer, rejects future sends and continues reading. Local close cancels connecting/read/write work and waits for normal shared cleanup; errors are actual typed SDK/IO failures, not invented HTTP handshakes.

## Mature libraries and modules

Tokio TCP/IO, tokio-util BytesCodec/LinesCodec/LengthDelimitedCodec, bytes/base64/hex, tokio-rustls/rustls native roots and existing signature-validating unverified-certificate adapter. Shared checked_destination resolves once and supplies only validated/pinned SocketAddr values to connect; no second DNS, proxy, redirect or external source download. No custom binary parser or TLS state machine.

Core config/encoding/URL validation in crates/core/src/tcp.rs. Runtime/control and bounded measured IO in crates/protocols/src/tcp.rs. Existing session owner/admission/logout/delete/lifetime/event retention covers TCP. React/Radix/CodeMirror module web/src/features/tcp with framing/payload/session/half-close controls and shared event viewer. Native/formats export handles drafts/privacy without changing database schema or source codec semantics.

## Bounds and privacy

Max frame1MiB, command/payload source2MiB, decoded send1MiB, input/wire20MiB, explicit512 send commands/4096 receive frames, existing32 command queue/4 owner live sessions/256 retained events and30 minute lifetime. Connect timeout uses request1..120000ms; sends/half-close5s with cancellation; idle0(disabled) or100..120000ms. Mature codec limits bound line/frame buffering; declared oversized lengths are rejected before receiving their payload. Raw mode reports observed stream chunks, not peer messages.

Known scoped private values and explicit secret send taint TCP session payload visibility conservatively: all subsequent byte payload events are withheld, retaining byte counts/direction/state. This prevents binary/fragmented echoes from bypassing text redaction. No payload value enters send-error text or logs. Default export screens editable drafts and excludes secret/unsafe binary payloads; encoded payloads are retained only when decoded JSON is unchanged by privacy screening (or an exact unresolved variable reference); other opaque encoded data is withheld. explicit include-secrets restores exact originals. No history persistence or automatic files/resource fetching.

## Seams and success criteria

Use actual TCP/TLS fixtures through owner-bound session API; verify byte-perfect text/Hex/Base64, fragmented/coalesced lengths and CRLF lines, TCP write EOF/continuing read, queue/command/frame/wire/idle limits, cancel while waiting, DNS/private policy, actual TLS rejection/explicit skip, wrong-owner/delete cleanup, private echo withholding, original payload interpolation/native source roundtrip. UI guards must prevent a pending send/result/template change attaching across request/account/workspace switch. Browser actual send/results/half-close/close and390px light/dark accessibility/overflow; compiled server embeds current UI. No local Docker; main integration/distribution refinement only after parent functional completion.

Not covered by this slice: arbitrary user framing scripts, custom length offsets/types, packet capture, automatic reconnect, mutual TLS/client certificate manager, post/event scripts, durable shared stream histories. Keep these gaps explicit; complete TCP basic-client evidence is not full product parity.

## Commands

`cargo test -p moleapi-core tcp --locked`; `cargo test -p moleapi-server --test tcp --no-default-features --locked`; `cargo clippy --all-targets --no-default-features --locked -- -D warnings`; explicit-package cargo fmt; `npm --prefix web test`; `npm --prefix web run build`; agent-browser named-session actual hosted fixture flows.
