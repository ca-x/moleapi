# MoleAPI Socket.IO SDK patch

Source: crates.io `rust_socketio` 0.6.0, repository https://github.com/1c3t3a/rust-socketio, upstream revision `3434b654c18580785c0d0171bc1acbc0378580c7`, package path `socketio`. Original Cargo manifests and VCS metadata are retained. MIT license copied from the upstream root LICENSE (copyright Bastian Kersting).

The SDK continues to own Socket.IO/Engine.IO parsing, serialization, heartbeat, namespace state and acknowledgement routing. MoleAPI does not serialize a parallel wire protocol.

Scoped additions:

- `ClientBuilder::connect_with_engineio`: inject a checked Engine.IO client, start no unowned background poll task or automatic reconnect.
- Public decoded SDK `packet` types and `Client::as_stream`: caller drives and cancels SDK packet processing.
- `emit_arguments`, `emit_arguments_ack`, `reply_ack`: positional arguments with all binary attachments, SDK-created packet types and identifiers. ACK requests enroll and send in caller order with monotonic IDs, SDK oneshot routing, timeout classification and a drop guard that removes callbacks on success/error/timeout/cancellation.
- Binary packet data remains the full JSON array with placeholders. Upstream's previous decoder removed only placeholder zero using textual replacement and discarded mixed/nested argument structure; its encoder also supported only a single binary argument. The SDK serializer now writes that preserved array.
- Packet-wide send locking keeps headers and all attachments together under concurrent emits; Engine.IO heartbeat packets are handled correctly between inbound attachments.
- Attachment count/aggregate payload bounds and clean incomplete-packet errors replace an upstream EOF `unwrap`.
- Rust cfg/lifetime/dead-code warning compatibility annotations only.

Current capability: Socket.IO revision 5 / Engine.IO revision 4, verified against official Node Socket.IO 4.8.1. WebSocket only in MoleAPI. Polling, upgrades, legacy Engine.IO 3 / Socket.IO 2 and automatic reconnect are not exposed. Caller uses a URL retaining its original hostname with an already policy-checked pinned TCP stream; TLS Host/SNI/certificate checks stay with mature tungstenite/native TLS.
