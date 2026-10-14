# Native CLI streamed octet-stream file input

Parent: code-generation.md and milestone2.

Replace whole-file binary input with Tokio ReaderStream and a small http_body adapter
that exposes the opened regular file's exact remaining size. Cap reads to the captured
length; shrink before completion returns UnexpectedEof, growth is not appended, and
non-regular files reject. Preserve known Content-Length, byte fidelity, zero-byte files,
optional omission and required CLI semantics. Reader capacity64KiB. Text input remains
validated/buffered UTF8; WebSocket file input remains one buffered binary message.

Finish implementation and tests before executing: generated standalone96MiB byte/digest
upload to a bounded-memory slow server and observed Linux RSS ceiling64MiB, generated
helper size/shrink/growth/directory tests, plus existing optional-body/binary/upgrade/
pagination generated-program regressions. Test owned-worker/privacy artifact behavior.
Run scoped generation/API/lint/format gates, document exact evidence/remaining transports,
commit/merge/push verified work.


Verified:96MiB upload length/digest matched, final observed Linux RSS12,940KiB below64MiB;
helper empty/growth/shrink/regular-file tests passed. All6 standalone runtime fixtures
passed39 requests;71 generation regressions and11 owned API cases passed with strict
scoped Clippy/formatting. The representative native3.0 CLI catalog status is updated.
Remaining streaming/transports/platform requirements remain explicit in parent coverage.
