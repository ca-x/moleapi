# Native generated raw request body semantics

Parent: code-generation.md and milestone2.

Preserve upstream operation parsing and transport. Carry the original requestBody.required
flag into operation metadata. For raw bodies only, optional positional APIs accept Option,
optional builder bodies default to None and body setters set Some. Set Content-Type/body
only when provided; omission and a supplied zero-byte file remain distinct. Required bodies
keep their existing validation. CLI --body-file requiredness follows source metadata;
text bodies read UTF8 strings and fail before network on invalid UTF8; octet streams read
exact bytes. This slice keeps buffered file input; streaming transport remains separate.

Write implementation and fixtures first. Verify generated CLI compile and wire cases for
optional binary/text omission, explicit empty/provided files, required binary/text omission,
invalid text/missing files, plus positional SDK compilation and calls. Cover original-source
privacy/owned-worker API and binary/upgrade regressions. Run checks only after source/tests
are complete, document actual evidence and commit/merge/push verified work.


Verified:2 targeted artifact/process cases passed;9 actual CLI/positional SDK requests
plus5 binary and6 upgrade/WebSocket regressions passed. Full generation68 cases and
11 owned-worker API cases passed with scoped strict Clippy/formatting. Buffered file
input and optional typed-body semantics remain explicit open scope.
