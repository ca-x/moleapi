# gRPC frontend / interchange independent review

Baseline: `d0fd769`; reviewed current uncommitted dedicated gRPC UI, protocol session hook, shared editor/models, and native/foreign interchange. Source files were read only. Review report is the only workspace file written by this reviewer. No commits, services, QA accounts, fixtures, or browser sessions were changed.

## Findings

| Severity | Before | After / required correction | Why and reproduction evidence |
| --- | --- | --- | --- |
| P1 | Default MoleAPI export redacts HTTP body JSON, but leaves `Protocol::Grpc.message_source` untouched. | Scrub sensitive JSON fields in the authoritative gRPC draft when `include_secrets=false`, preserving schema sources; verify shared snapshots through the same export path. | `crates/formats/src/redact.rs:43` redacts `request.body` only. Save a gRPC draft `{"password":"literal-secret","token":"literal-token"}`, export MoleAPI without secrets: both remain in `protocol.message_source`. These are credentials explicitly scrubbed by existing HTTP-body policy. |
| P2 | Stop is missing during initial session creation and `close()` ignores a pending creation without a session ID. | Render Stop during pending creation and invalidate that operation so its late result closes without activating or applying private updates. | Initially `GrpcWorkbench.tsx` renders Stop only for `active`, and `useProtocolSession.ts:209` returns on missing session ID. Delay `/api/sessions` after Call: loading state offers no cancel action. Root has applied an initial correction. |
| P2 | Initial cancellation correction fences only when no session ID exists; a retained terminal session prevents cancellation of the next call. | Fence the current pending operation before handling any retained session ID. | Complete a call, click Call again with delayed POST, click Stop: `close()` sees the old terminal session ID and closes it, then the late new session still activates. Independently reread corrected `useProtocolSession.ts:208` and sent root this follow-up. |
| P2 | Proto validation captures a snapshot while name/path/content/import/add/remove/entry controls remain editable; success closes the dialog using the older snapshot. | Freeze source edits while validating/picking, or version edited state and retain newer edits without closing. | Delay `/api/grpc/import`, validate a valid proto, edit its message/name before response, then return success. `ProtoSourceDialog.tsx:119–141` attaches the earlier payload and closes, silently discarding the visible new draft. Reproduced in isolated copied frontend with a deferred API response. Root has disabled mutation controls and made the editor read-only while busy. |
| P2 | Foreign export rejects gRPC requests but ignores retained protobuf sources when remaining requests are HTTP. | Reject Postman/OpenAPI when any protobuf specification would be lost, recommending MoleAPI. | Import a proto source, switch its request to HTTP, export Postman/OpenAPI. Original `formats/src/lib.rs:53` visits protocols only; exporters omit protobuf definitions. Root has added a protobuf-specification guard. |

## Verification and covered surface

- Read `docs/specs/grpc-client.md` and implementation plan; reviewed source attachment preserving other definitions, account/workspace/request/environment fences, dialog close/reopen epochs, schema reopen path, typed sends/half-close, terminal events, mature JSON/protobuf editor assets, and native source retention.
- Ran `npm --prefix web test -- --run src/features/grpc src/features/protocols/lifecycle.test.tsx src/features/protocols/events.test.ts`: **13 passed, exit 0** during review.
- Isolated copied frontend at `/tmp/grpc-ui-review-13szcE` contains reviewer-only deferred-response probes. Proto edit-loss probe reproduced the issue. Initial no-session cancellation probe expected the old bug but instead confirmed root's cancellation correction had arrived before the copy: late session was closed and no live session remained. This probe failure is not a product-test failure; it reflects changed source during active review.
- No new live-browser transport claim: root owns the real tonic/browser acceptance. Existing schema and lifecycle tests cover late environment/unmount/auth changes. Transport/server internals are assigned to the independent Rust review.
- Native proto/descriptor sources survive existing structured source redaction because canonical `files[].content` and `descriptor_set_base64` keys are not credential-key matches. Native export privacy for request drafts is the additional issue above.

## Correction recheck at 13:51 UTC

Root corrected initial and retained-terminal cancellation: the pending-operation generation fence now runs before any session-ID branch; Stop is visible for pending creation. Re-ran targeted frontend command above: **15 passed, exit 0**, including both fresh and retained-terminal cancellation regressions. Proto edit mutation controls are disabled/read-only while busy, preserving close/reopen fencing. Foreign exporter now explicitly rejects retained protobuf sources.

`cargo test -p moleapi-formats --test interchange` failed to compile root's newly added source-retention regression: `interchange.rs:219` used `unwrap_err()`, which requires `ExportResult: Debug`. Sent root the exact error and recommended `.err().expect(...)`. This is a test-harness compile issue pending correction, not evidence that the export guard failed behaviorally.

Native `protocol.message_source` credential redaction remains pending at this checkpoint. No full-ready verdict until that finding and interchange regression compilation are resolved.


## Final independent recheck at 13:55 UTC

All four underlying findings are corrected in the current source. Read the final `useProtocolSession.close`, `GrpcWorkbench` Stop condition, busy proto form/picker/editor controls, exporter protobuf guard, and protocol JSON redactor. The pending-generation fence now precedes retained session-ID handling. Canonical proto/descriptor source retention remains intact. Default export scrubs nested gRPC message credentials; explicit include-secrets export retains them. Malformed protocol JSON is withheld as `{}`. GraphQL variables/raw drafts/connection parameters and ws/wss credential URLs now receive equivalent screening.

Ran fresh commands independently after corrections:

- `npm --prefix web test -- --run src/features/grpc src/features/protocols/lifecycle.test.tsx src/features/protocols/events.test.ts` → **16 passed, exit 0**, covering fresh and retained-terminal pending cancellation, proto lock during validation, discarded dialog results, environment late Reflection, authentication expiry, event retention, and path handling.
- `cargo test -p moleapi-formats --test interchange` → **10 passed, exit 0**, covering the previously failing regression, retained protobuf source rejection/native round-trip, nested protocol draft redaction, and explicit-secret preservation.

No remaining actionable frontend/interchange finding in the reviewed slice. This scope verdict does not replace root's real-service/browser acceptance or independent Rust transport review. Source files stayed untouched by this reviewer; only this report was updated. No commits or public actions.
