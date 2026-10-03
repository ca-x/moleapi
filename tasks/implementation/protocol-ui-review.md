# Protocol frontend and interchange review

Review baseline: `11250e1f4487a33a9ffd5c271e7e3d8878f240bc` plus the current uncommitted changes, including all five new `web/src/features/protocols` files. Review date: 2026-10-03. Read-only source review; only this requested report was written. Backend security is a separate review scope.

## Fix round 1 recheck

Current status: all three original P2 findings are resolved. No additional actionable frontend/interchange finding was identified in the targeted recheck. This is a scoped review conclusion; browser/mobile QA and packaging remain root-owned gates.

- Session creation now captures a generation ticket and requires a mounted, current scope both after the save wait and after creation. A late successful response closes the returned session before any local application. The unmount cleanup invalidates the generation. Closed/sent callback completions also require the current generation and session ID before updating state, refetching or displaying errors.
- The controller passes actual authentication state into the hook. Authentication is part of its identity, live-session visibility and query enablement. Expiry immediately hides the session and disables polling. Query execution checks current scope/session before its first call and after each await, so late status results cannot start an old-session events call and late batches cannot install old state.
- Request generation distinguishes leaving and reselecting the same identity. An independent deferred-create reproduction verified that A → B → A does not revive the first A creation or apply its local updates.
- The server now accepts retained draft body text when the execution body mode is None. `prepare_live` keeps the saved/script-visible draft and clears only the execution clone before interpolation; unresolved templates in ignored draft text do not block connection. The API and engine validate GET/None after scripts. The exact integration fixture verifies real SSE and WebSocket handshakes with empty wire bodies, saved draft preservation, no fabricated request update, and rejection of script mutations to POST or an active body mode.
- Frontend session/event/send JSON still matches the Rust models; the fix did not change those API shapes.

Independent verification during this recheck:

```sh
# Repository regressions: exit 0, 2 passed.
cd web
npm test -- src/features/protocols/lifecycle.test.tsx

# External harness: exit 0, 4 passed. Covers the two original lifecycle failures,
# A → B → A creation fencing, and old-status completion after a scope switch.
cd /tmp/moleapi-protocol-ui-review
/home/czyt/code/rust/moleapi/.worktrees/application/web/node_modules/.bin/vitest run --config /tmp/moleapi-protocol-ui-review/vitest.config.mts --reporter=dot

# Exact real SSE/WebSocket retained-body fixture: exit 0, 1 passed.
cd /home/czyt/code/rust/moleapi/.worktrees/application
cargo test -p moleapi-server --test protocols retained_none_body_is_not_sent_and_script_method_or_body_mutations_are_rejected -- --exact
```

Source files were not changed by the reviewer. The original failing-test evidence below describes the pre-fix state; it is retained to document why each finding was raised.

## Original findings (resolved in fix round 1)

### P2 — Session creation completing after unmount leaks the new session and applies local updates

Locations: `web/src/features/protocols/useProtocolSession.ts:63` and `:135`.

Cleanup closes only the `sessionId` captured by an already committed render. While `POST /api/sessions` is pending, that ID is absent. Unmount does not invalidate `identityRef`, so a successful response arriving afterward passes the identity comparison, calls `locals.apply`, and installs a session in an unmounted hook. No close request is made. The remote worker can remain live until its backend expiry instead of being cancelled with its UI owner.

Reproduction: mount the hook with one saved SSE request and a deferred create API response; call `connect()`; unmount before resolving the create; resolve with a valid open session and one variable update. An external React Testing Library/Vitest reproduction observed `locals.apply` called once and exactly one API call (the create), with no `/close` call. Expected: invalidate the creation lifetime and close the late-created session without applying updates.

The existing account/workspace/request comparison handles a changed identity but does not handle an ended mount lifetime. Use a mounted/generation fence in addition to identity, including the save wait and creation response paths.

### P2 — Authentication expiry leaves the live-session poll timer running behind the login screen

Locations: `web/src/features/protocols/useProtocolSession.ts:74` and `web/src/features/workbench/useWorkbenchController.ts:54`.

The controller always mounts the protocol hook and passes no authenticated state. Its query is enabled solely by `sessionId`. `useAuth` responds to `moleapi:unauthorized` by clearing the token and setting `authenticated=false`; it leaves the account ID and workspace draft unchanged. `App` then shows the login screen, but the controller/hook remains mounted and its identity remains valid. The last open state continues to schedule polls every 750 ms. Real requests will repeatedly return unauthorized; `retry:false` does not disable the separate refetch interval.

Reproduction: mount the real workbench controller with a token, saved workspace and open SSE session; connect and allow one status/events poll; dispatch `moleapi:unauthorized`; verify `authenticated=false`; wait 850 ms. The focused reproduction observed session status calls increase from one to two after authentication became false. Expected: clear or disable the live execution scope and stop polling immediately on authentication loss, with stale creation/poll results fenced from local mutations.

The explicit logout action eventually clears the draft, but the auth-expiry/unauthorized path does not use that action. This is a frontend lifecycle issue, not an ownership bypass finding.

### P2 — Switching an HTTP request with body text to a live protocol creates an unconnectable hidden body

Location: `web/src/features/requests/RequestEditor.tsx:82` (protocol-change patch). Consumer: `crates/server/src/protocols.rs:76`.

Protocol selection changes the method to GET and body kind to `none`, but retains `request.body`. The editor consequently hides the body editor and states that no body is sent. `connect()` clones and submits the entire request unchanged. `resolve_request` retains the body string, and session creation rejects any nonempty body even when `body_kind` is `none`.

Reproduction: create an HTTP request with JSON body `{"hello":"world"}`; select SSE or WebSocket; point it at a suitable endpoint; click Connect. Creation returns `SSE and WebSocket connections require GET with an empty body`. The same issue affects a request that previously had body text and is already set to “no request body.” Expected: align the outgoing live snapshot and editor state with the empty-body contract; if retaining the HTTP body for switching back, keep it out of the live execution payload or offer an explicit visible resolution.

This finding is supported by direct tracing of the selector patch, `connect()` snapshot, `resolve_request`, and the creation guard; no backend fixture was claimed for this reproduction.

## Other review observations

- TypeScript session, event and send JSON shapes match `crates/protocols/src/models.rs`; protocol configuration matches the serde tagged enum in the core model.
- Events preserve cursor ordering and data, distinguish incoming/outgoing/system direction, and bound the displayed log to 256 records and 8 MiB of UTF-8 JSON. SSE ID/retry, Base64 binary/ping/pong, close code/reason and handshake metadata remain inspectable.
- Normal identity changes hide previous-account/workspace/request events and reject late creation results. The lifecycle findings above are gaps in that otherwise useful fencing.
- The implementation uses existing Radix selection/tabs, TanStack Query, CodeMirror and Lucide. Repeated event arrivals and send shortcuts introduce no animation.
- HTTP continues to render the existing finite response pane. The live request editor still exposes the HTTP assertions/examples editors; an explanatory unavailable-state message would make the limited live capability clearer. This is advisory because the code does not fabricate assertion results.
- MoleAPI export/import preserves the protocol discriminator. Other supported export formats explicitly reject protocol loss. No additional interchange correctness finding was identified.

## Verification and limits

Focused external harness: `/tmp/moleapi-protocol-ui-review/lifecycle.test.tsx`, with config `/tmp/moleapi-protocol-ui-review/vitest.config.mts`. Command:

```sh
cd /tmp/moleapi-protocol-ui-review
/home/czyt/code/rust/moleapi/.worktrees/application/web/node_modules/.bin/vitest run --config /tmp/moleapi-protocol-ui-review/vitest.config.mts --reporter=dot
```

Result: exit 1, two tests fail on the intended lifecycle expectations: local updates occur after unmount, no late-session close is called, and polling continues after unauthorized authentication loss. API responses are mocked to deterministically isolate React/controller timing; these tests are not evidence of a real network connection.

Existing 29 frontend tests/build/typecheck and seven interchange tests were reported by the implementer; they were not rerun or represented as independent evidence here. Root owns real SSE/WebSocket/browser/mobile/a11y QA, packaging and release gates. Its browser session, servers and fixtures were not changed. No full suite, commit, push or subagent was performed.

Completion ledger:

1. Frontend/API-contract/event-retention/identity/lifecycle surface: reviewed; three original P2 findings resolved in fix round 1; one capability-clarity advisory.
2. Export guard and protocol roundtrip test: reviewed; no additional finding.
3. Backend security, real browser fixtures and packaging: outside this assigned review; no sign-off claimed.
