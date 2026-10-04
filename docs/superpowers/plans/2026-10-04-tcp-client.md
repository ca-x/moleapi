# TCP client implementation plan

Spec: docs/specs/tcp-client.md. BASE2313186; existing worktree/authorization, inline implementation, fresh final scoped review. Feature-first; no local Docker/main scaffold mutation.

Task1: core TcpConfig/TcpMessage/URL/encoding validation and preserved raw draft interpolation. Public model/validation regression RED→GREEN. Add protocol actor/session commands/configuration and measured IO with official codecs and checked TCP/TLS, bounded send/half-close, shared admission/ownership/cancel/privacy. Real owner-bound fixture regression RED→GREEN for framing/binary/half-close/TLS/quotas/cleanup/privacy. Format native default/full source tests.

Task2: mature lazy TCP workbench/settings/payload/privacy/status/event viewer, core/shared selector/types integration and raw-transport unsupported-control treatment. UI lifecycle/model tests, actual browser TCP fixture/half-close/Stop/narrow a11y. Final source/runtime/embed/strict gates, fresh scoped review, fixes with regression evidence, docs/proxy commit. Parent full feature work remains active.

Interfaces: provider core TcpMessage encoding/payload_source/secret and TcpConfig framing/max_frame_bytes/no_delay/idle_timeout_ms/message; sessions tcp_send{message} and tcp_half_close; Event tcp_data{base64,text,bytes,redacted},tcp_half_closed. UI consumes exact DTO and no invented framing/parser. Saved message source is opaque to generic transport interpolation, resolves only at explicit send.

Tasks1/2 implemented and verified. Fresh final review plus one implementer regression fix pass, actual browser/source/strict/embed tests recorded in IMPLEMENTATION-STATUS and tcp-review.md. Parent feature work remains active; native/latest distribution gates deferred.
