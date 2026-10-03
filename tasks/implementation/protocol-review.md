# Independent protocol backend review

Review baseline: `11250e1f4487a33a9ffd5c271e7e3d8878f240bc`; reviewed uncommitted backend changes in `tasks/implementation/protocol-review.diff` against actual source. Scope: new protocol crate, shared core transport/model/validation changes, session APIs, pre-script/privacy handling, authentication cleanup, workspace reconciliation, native sync acknowledgment, and associated tests. Frontend, formats, workflows, packaged desktop builds and external database matrix remain controller-owned. This is a deep scoped review using the check skill; no subagents or source modifications were made.

Verdict: **three P2 findings require fixes**. These are independently reproduced behavior, not speculative cross-account attacks.

## P2 — Capture original request credentials before running pre scripts

Source: `crates/server/src/protocols.rs:67` calls `prepare_live` before credential collection at lines77–98. `crates/server/src/execution.rs:270` masks failed scripts with the worker's private-values set, which initially contains variable secrets but not literal request credentials. On success, only the modified request's remaining credentials enter the redactor.

Two concrete triggers using an enabled `authorization: Bearer review-fake-private-credential` request header:

1. `throw new Error(pm.request.headers.get('authorization'));` returns HTTP400 with `{"error":"JavaScript error: Bearer review-fake-private-credential"}`.
2. `console.log(pm.request.headers.get('authorization')); pm.request.headers.remove('authorization');` successfully creates a session whose retained `script_log` contains `Bearer review-fake-private-credential`.

The late redactor cannot fix either case: failure exits before it exists; successful header removal erases the only source from which it would learn the value. Capture original auth/sensitive header values into `scopes.private_values` before invoking the worker, retain that set across mutations, and merge final resolved credentials afterward. Add both failure and remove/replace-success regressions, including an encoded form. This is a metadata redaction violation within the authenticated owner's response/event log; no cross-owner disclosure is claimed.

## P2 — Include explicitly secret query values in metadata redaction

Source: `crates/server/src/protocols.rs:89` collects only sensitive headers; `crates/server/src/protocols.rs:122` appends every enabled query pair, and lines133–136 mask using a private-values set that never includes query values marked `secret:true`. `crates/core/src/redaction.rs:3` has no access to pair flags; it only sees variable secrets and heuristic query-key names.

Trigger: create an SSE request with `query:[{"id":"q","key":"opaque","value":"review-query-secret","enabled":true,"secret":true}]`. The creation response's `url` is `http://127.0.0.1:<fixture>/events?opaque=review-query-secret`. Thus an explicit secrecy flag is ignored when the parameter name does not happen to match the key-name heuristic. Collect enabled secret query values (original and resolved) into the same retained privacy set before summaries, feedback and errors are formed. Test an innocuously named query field and an encoded value.

## P2 — Fence session admission against logout during preparation

Source: `crates/server/src/auth.rs:204` calls `SessionManager::close_owner`; `crates/protocols/src/lib.rs:418` snapshots only already-registered IDs. `crates/server/src/protocols.rs:68` waits for pre-script execution before registration at line127; its final `check` at line141 verifies workspace/request membership, not whether the authentication session was revoked.

Reproduced sequence: start a create request with a150ms bounded synchronous pre script;50ms later call logout with the same token. Logout returns200 before creation returns200/`connecting`. Re-login and GET the returned protocol session ID shows `open`; the fixture is receiving the new connection after logout. The worker report already acknowledges this limitation, but it violates this task's explicit logout cleanup contract. The existing logout integration waits for `open` before logout and therefore misses this admission race.

Fence preparation/registration with logout using a revocation generation or equivalent atomic admission check and cleanup, so creation admitted before logout cannot register a fresh worker afterward. Preserve valid independent logins according to the intended owner/session cancellation policy. Add a coordinated test that pauses preparation, completes logout, then releases preparation and asserts no live network connection survives. Owner isolation and the30-minute lifetime do limit impact; this is incomplete cleanup, not an authentication bypass to another account.

## Independent verification

Executed `python /tmp/moleapi-protocol-independent-review/repro.py` against the already-built worktree `target/debug/moleapi-server`, with its own temporary SQLite database, hosted server on an ephemeral loopback port, and an independent Python SSE fixture on another ephemeral port. The harness terminated its server and fixture and removed its temporary database. It did not use the controller's18877/18879 servers, browser, or accounts. Values named `review-*` above are synthetic test credentials.

Observed output:

```text
FAILURE_REDACTION 400 {"error": "JavaScript error: Bearer review-fake-private-credential"}
REMOVED_HEADER_LOG ... "message": "Bearer review-fake-private-credential"
QUERY_SECRET_SUMMARY 200 http://127.0.0.1:40171/events?opaque=review-query-secret
LOGOUT_RACE 200 200 connecting
SESSION_AFTER_LOGOUT open
```

Read the real fixture tests and checked upstream callers/downstream consumers for each finding. No full Rust test suite was repeated, as requested. The worker-reported59-test and subsequent42-test runs, clippy, formatting and native library checks were not independently rerun and are not represented as this review's test execution. The reproduction uses the existing binary, with source inspection corroborating each observed path.

## Reviewed constraints and limits

- Mature SSE parser and WebSocket handshake/framing libraries are wired to checked/pinned reqwest destinations. TLS verification options, proxy disabling, per-hop address checks, downgrade rejection and cross-origin Authorization/Cookie removal remain present. No new unchecked DNS socket path or native TCP listener was found.
- Old RequestSpec JSON defaults to HTTP; HTTP execution rejects live protocols. Native IPC forwards JSON routes through the same router. Sync push compares returned name/data before acknowledging a baseline; the old-server data-loss fixture targets that path.
- Session methods check owner and existing workspace/request membership. Connecting registration precedes final membership validation; workspace/request deletion reconciles and cancels registered workers.
- Message/frame, outgoing/incoming bytes, commands, event retention, owner/global admission, lifetime and terminal retention bounds are present. Cursors distinguish gaps and reject future positions. Close waits for completion; worker failure becomes error. No session event writes to history/sync were found.
- Raw protocol payloads are deliberately owner-visible volatile data. The findings concern summaries/script metadata and lifecycle, not a demand to redact all payloads.
- The conservative SSE pending-wire limit and absent reconnect/replay/per-event post scripts are explicitly documented and outside the findings. Real TLS certificates, production deployments, three external database engines and frontend account-fencing behavior were not exercised here.

Completion ledger: scoped backend review done; independent reproduction done; report written; fixes and targeted regression verification pending controller/implementation worker. No commits, pushes, source edits or public actions.
