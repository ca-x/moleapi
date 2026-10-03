# Fresh Socket.IO frontend/interchange review

Reviewed uncommitted application worktree against base `0a64ae6` on 2026-10-03. Read Socket.IO spec, implementation plan and backend report. Read-only on application source; only this report was written in the worktree. No account/browser/server fixtures were changed. Root owns actual browser and narrow-layout QA.

Two actionable findings.

| Severity / evidence | Before | After | Why |
| --- | --- | --- | --- |
| P2 — `web/src/features/socketio/SocketIoWorkbench.tsx:125` | A successful ACK reply updates `answeredAcks` only when `replyRef.current === token`. Closing the reply dialog while the request is pending clears `replyTarget`, so successful completion never marks the consumed callback answered. | On same-mounted-scope success, always add the original token to `answeredAcks`; separately close the modal only if it still shows that token. Add a cancellation-during-send regression. | The callback is exactly once at the server, independently of whether its dialog remains open. Reopening the event currently offers a second reply and produces an expired/already-answered rejection. Keep scope/session fences and preserve a newer dialog. |
| P2 — `crates/formats/src/redact.rs:159` | Default native export clears a sensitive JSON key only when its value is a string. Auth `{"token":123456}` and outbound `[{"password":654321}]` remain intact in an export with `include_secrets=false`. | Withhold the entire value of sensitive JSON keys regardless of its JSON type, including numbers and containers; preserve explicit include-secrets roundtrips. Add numeric auth/argument export regression. | Socket.IO auth is arbitrary JSON and the backend already supports numeric credentials. A default export currently shares these literal credentials despite its privacy contract. |

## Reproduction and verification

ACK race: independent temporary React/Vitest harness at `/tmp/moleapi-socketio-ui-review/ack-race.test.tsx` imports the actual workbench and supplies a controllable send promise. Open incoming callback reply, click Reply ACK, dismiss with Cancel, resolve send successfully, and assert the event reply button is disabled. Actual result: `expected false to be true`, 1 failing regression. The component uses real Radix dialog controls; the mocked event pane exposes the actual `canReply` result. No repository test/source was edited.

Numeric export: temporary Rust executable source `/tmp/moleapi-socketio-ui-review/numeric-export.rs` links the existing compiled `moleapi_formats` library. It builds a real imported request with Socket.IO auth `{"token":123456}` and arguments `[{"password":654321}]`, calls `export(&workspace, "moleapi", false)`, and checks absence of those values. Actual exported protocol contains both numeric values and the assertion fails with `numeric credentials escaped default redaction`.

Existing targeted suite passes: `npm test -- src/features/socketio/messages.test.ts src/features/socketio/SocketIoWorkbench.test.tsx src/features/protocols/events.test.ts src/features/protocols/lifecycle.test.tsx` — 4 files, 11 tests. This verifies fresh outbound correlation IDs, nested positional JSON and binary attachment preservation, explicit `""` zero-byte attachment ordering, newer payload preservation during an asynchronous listener update, late-unmount cleanup/auth loss, and pending Stop with/without retained terminal sessions.

Static review found no additional concrete defect in saved canonical fields, command typing, owner/account/request/environment/session fences, delayed listener payload preservation, event/ACK argument rendering, terminal reason handling, credential-string redaction, foreign-format rejection, accessible control labels or shared narrow event layout. ACK status retains successful application-error payloads and does not conflate them with transport failure. Actual fixture send/listen/reply and narrow accessibility validation remain root-owned; this review does not claim independently performed browser coverage.

## Read-only recheck after root fixes

Original two P2 findings are resolved in the inspected source and independent reproductions: the temporary cancellation race now passes, and the recompiled actual default export replaces both numeric credentials with null. Updated targeted UI suite passes 12/12 tests across 4 files, including root's retained ACK regression.

One new regression from splitting payload/schema redaction remains:

| Severity / evidence | Before | After | Why |
| --- | --- | --- | --- |
| P2 — `crates/formats/src/redact.rs`, new `redact_payload` traversal | Payload traversal no longer applies the former `url`/`raw` string handling. Default export of Socket.IO auth `{"url":"https://user:url-password@example.com?token=query-token"}` contains both credentials unchanged. | Restore `url`/`raw` string normalization through `redact_url` followed by `redact_embedded_json` within payload traversal, keeping the new type-independent sensitive-key handling and separate schema traversal. Add a URL-in-payload export regression. | The old shared traversal scrubbed URL userinfo and sensitive query values inside protocol JSON, HTTP body and example payloads. The fix removes that protection from every payload caller. |

Reproduced against the current compiled formats library with `/tmp/moleapi-socketio-ui-review/url-export.rs`: actual output retains `user:url-password` and `token=query-token`; assertion fails with `URL credentials escaped default redaction`. No application source changes by reviewer.

## Final recheck — all findings resolved

Root restored payload `url`/`raw` redaction while retaining type-independent sensitive-key redaction and schema preservation. The independent URL export executable was rebuilt against the newest compiled formats library and now exits 0; actual output is `https://example.com/?token=`. The independent numeric export executable still exits 0 with null credential values. The independent ACK cancellation/success race passes again.

Final review-owned verification: focused UI suite 12/12 passes across 4 files; newest compiled interchange integration test executable `target/debug/deps/interchange-18992bfb2fd173cf` passes 13/13, including `payload_url_and_raw_strings_keep_prior_credential_redaction` and `default_protocol_export_scrubs_non_string_credentials_without_erasing_schema_definitions`.

Open actionable findings: **0**. All three reproduced review findings are resolved. Scope remains the frontend/interchange ordinary functional review described above; root owns final complete build/typecheck and actual browser/narrow/accessibility QA. Reviewer made no application source changes.
