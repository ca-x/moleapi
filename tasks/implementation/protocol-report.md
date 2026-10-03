# Protocol session backend report

Worktree: `/home/czyt/code/rust/moleapi/.worktrees/application`, branch `feat/application`, implementation baseline `11250e1`. Changes remain uncommitted. Frontend, formats export policy, distribution, branding and workflows belong to the controller. Existing untracked draft artifacts were preserved. No local Docker commands were run.

## Implemented

- Additive `RequestSpec.protocol`: `{ "kind": "http" | "sse" | "websocket" }`. Missing fields deserialize to HTTP; existing HTTP execution rejects live protocols with a concrete session-API error. Workspace validation and interpolation support protocol-specific URL schemes.
- New `moleapi-protocols` crate: real reqwest streaming with `eventsource-stream` 0.2.3; WebSocket upgrade through `reqwest-websocket` 0.5.1 and mature bounded tungstenite framing. The adapter uses the checked/pinned shared reqwest client, retaining rustls, certificate verification options, no-proxy policy, per-hop DNS checks and cross-origin credential stripping. There is no unchecked URL-based WebSocket DNS connection or custom protocol grammar/crypto.
- Volatile owner-bound sessions, explicit connecting/open/closed/error states, safe handshake metadata, timestamped incoming/outgoing protocol messages, close metadata and ordered retention cursors.
- JSON create/read/events/send/close/delete routes in the same protected router used by hosted HTTP and native Tauri in-process IPC. No native TCP listener was introduced.
- Project/collection/request pre scripts run in the existing isolated worker. Their logs/tests become typed system events. Creation-only updates omit private/local values so those values cannot enter saved drafts. Saved project/collection/request post scripts are explicitly rejected for live sessions pending a per-event contract.
- Raw and resolved original auth/header/query credentials are captured before pre scripts, retained after removal/failure, and augmented with mutated/resolved values afterward. Sensitive header additions are tainted immediately in the isolated worker, including intermediate values removed before export. Known private values, request credentials and encoded variants are masked in summaries, handshake headers, script feedback and terminal errors. Raw owner-visible protocol payloads remain inspectable in volatile events; sessions write no HTTP history or workspace/sync state.
- Admission: four live and sixteen retained records per owner; sixty-four live globally; additional global retained ceiling 1024. Command queue 32. Message payload <=1MiB, ping <=125 bytes; encoded bounds checked before decoding. Outgoing input and incoming WebSocket payloads each capped at20MiB. Ring retains <=256 events and <=8MiB serialized event bytes, evicts oldest and reports exact cursor gaps. Live lifetime30min; terminal retention10min, swept every30s and during operations.
- SSE total wire ceiling8MiB. A conservative pending-event wire guard caps bytes since the last parser-delivered event to1MiB plus one64KiB chunk; parser input is split to bounded chunks with cooperative yields. This closes the quadratic rescanning problem found by the oversized unterminated fixture while keeping the mature parser authoritative.
- Register connecting sessions before the final workspace/request ownership recheck. Concurrent deletion can cancel a registered connection; final failure removes it without starting network work. Workspace deletion cancels workers and removes their retained records. Request removal during workspace save/native sync pull also closes/removes affected sessions. Logout cancels owner workers and fences in-flight preparation with an owner generation gate. Hosted token validity is rechecked before preparation and at final admission; native uses the same owner generation fence. Last manager drop cancels workers; supervisor converts task failure into error state.
- Native sync push compares the returned shared workspace name/data with the sent payload before committing its baseline. An older server that silently discards protocol fields is rejected instead of acknowledged as synchronized.

## Public JSON interface

`POST /api/sessions`: `{workspace_id, request:RequestSpec, environment_id?:string, locals?:VariableUpdate[]}`. The supplied request ID must exist in the owner’s workspace. Both live protocols require GET and body_kind:none after pre scripts. Retained editor body text is ignored during execution and preserved in saved requests.

Summary: `{id,workspace_id,request_id,url,protocol:'sse'|'websocket',state,reason:string|null,created_at,updated_at,received_bytes,sent_bytes,event_count,handshake:{status,headers:Pair[]}|null}`. URL is the resolved, redacted endpoint including enabled query pairs. Creation may include `variable_updates`/`request_updates`; later summaries omit those creation-only fields.

`GET /api/sessions/:id/events?after=N`: `{events,next_cursor,earliest_cursor,dropped_count}`. `after` denotes the last consumed cursor; cursor0 requests the beginning. Future cursors are rejected. A stale cursor returns the retained ordered batch plus the exact dropped-event count.

Events: `{cursor,received_at,direction:'incoming'|'outgoing'|'system',message}`. Messages:

- `{kind:'sse',event,data,id,retry:number|null}`; retry is milliseconds supplied by the parser.
- `{kind:'text',text}`.
- `{kind:'binary'|'ping'|'pong',base64}`.
- `{kind:'close',code:number|null,reason}`.
- `{kind:'state',state,reason:string|null}`.
- `{kind:'script_log',level,message}`.
- `{kind:'script_test',test:TestResult}`.

`POST /api/sessions/:id/send`: `{kind:'text',text}` or `{kind:'binary'|'ping',base64}`, returns `{ok:true}` after queue admission. Actual transmitted messages are logged only after successful send. SSE is receive-only. `POST .../close` waits for worker termination and returns its terminal summary. `DELETE .../:id` closes/removes the record and returns `{ok:true}`. All operations check owner and existing workspace/request membership.

## Actual verification

- `cargo test -p moleapi-protocols -p moleapi-core -p moleapi-server --no-default-features`:59 passed at the first broad gate (19 core,8 protocols,32 server). All eight existing HTTP network/SSRF/redirect tests passed.
- After the final cleanup/sync regression additions: `cargo test -p moleapi-protocols -p moleapi-server --no-default-features`:42 passed (8 protocols,34 server), no failures. The six real protocol fixtures finished in0.54s; four API protocol integrations and eight sync integrations passed.
- Real fixtures cover SSE split chunks/multiline/id/retry, hostile unterminated pending buffer, total8MiB wire ceiling, cancellation while connecting/reading, WebSocket text/binary/ping/pong/peer close/client close, cross-origin redirect credential stripping, unsafe redirect schemes and private network denial for both live protocols.
- In-process router tests cover native text send/receive/close/delete, owner isolation across every session endpoint, missing request rejection, old JSON default HTTP, no history writes, scoped isolated pre scripts with masked metadata, post-script rejection, deletion cleanup, request-removal network termination, and owner logout network termination. The sync fixture mimics an old server discarding SSE protocol and proves rejection/no baseline acknowledgment with a conflict on retry.
- Unit tests cover cursor gap/order, event byte/count bounds, owner admission, registered-before-start cancellation, command capacity, total outgoing input, terminal expiry and shutdown cancellation.
- `cargo check -p moleapi-server --lib --no-default-features` passed. `cargo clippy -p moleapi-core -p moleapi-protocols -p moleapi-server --all-targets --no-default-features -- -D warnings`, `cargo fmt --all -- --check` and `git diff --check` all passed on the final source tree.

## Explicit limits and review notes

No reconnect/replay/durable session history, per-event post scripts, WebSocket subprotocol editor, or protocols beyond SSE/WebSocket are implemented. SSE comment-only/id-only streams can conservatively hit the pending-wire guard even though they do not deliver data events; the error names that limit. Received byte counters describe SSE wire bytes and WebSocket decoded payload bytes, excluding WebSocket framing. Retention is lossy and cursor gaps are explicit.

Logout/admission now share a bounded per-owner gate. Preparation captures the owner generation and validates the hosted auth token, releases the gate while scripts run, then reacquires it to recheck generation/token and hold the registration/final workspace ownership check/start boundary. Logout revokes its token, increments that owner’s generation and closes owner workers under the same gate before returning200. If logout wins, an earlier preparation returns401 without starting a connection; if admission wins, logout sees and terminates that registered worker. Weak gates are pruned when no pending call retains them, with an additional1024 concurrent-owner preparation limit. Unrelated owners have separate gates.

The sync acknowledgment check protects the local baseline and local workspace; a remote server may already have performed its incompatible write before returning the mismatching acknowledgment. Updating that remote is required, and there is no distributed rollback guarantee. This is additive new-reads-old compatibility, not a claim of mixed-version feature equivalence.

This worker verified native/no-default Rust library and in-process IPC behavior, not a new packaged Tauri executable or three independently configured external database engines. Browser UI, package builds, CI Docker and database service matrix are controller-owned gates.

## Independent review round1 fixes

The reviewer reproduced raw original Authorization in a failed pre-script error, logged Authorization after removal, an opaque explicitly-secret query in the summary, and an in-flight150ms script opening after logout50ms later. A new regression first reproduced the unmasked error before the fix. Raw/currently resolvable original credentials now seed the worker’s privacy set before scripts; success/failure unions retain those values, sensitive header mutations taint immediately, final mutated/resolved values augment them, and explicit query-secret flags participate in encoded redaction. A bounded core single-field interpolation helper reuses the established interpolation grammar so unrelated script-defined URL variables do not prevent early credential capture.

Added tests verify original Authorization throw/log/remove, public-scope template-resolved credentials after header removal, secret opaque query encoding, intermediate script-added Authorization log/remove/throw, and hosted/native logout during pre scripts. Both races return401, produce no fixture connection, and a fresh authenticated create still succeeds.

Fresh final functional gate: `cargo test -p moleapi-core -p moleapi-protocols -p moleapi-script-runtime -p moleapi-server --no-default-features`:75 tests passed (19 core,8 protocols,12 script runtime,36 server), zero failures. This includes all6 protocol API integrations, all6 real protocol fixtures, existing HTTP SSRF checks and native worker cancellation/deadline tests. Native no-default library compilation passed. Strict Clippy across all four crates/all targets passed after helper-ordering and nested-if style corrections. Formatting and diff whitespace checks passed. The reviewer’s exact `/tmp/moleapi-protocol-independent-review/repro.py` also exited0: failed pre-script400 message `[REDACTED]`, removed-header script log `[REDACTED]`, secret query summary `opaque=[REDACTED]`, and logout200 followed by in-flight creation401, with no live session returned.

## Frontend integration body-mode correction

Live connection validation now follows the body mode: None ignores retained editor draft text instead of rejecting it. The draft remains available to pre scripts and untouched in saved workspaces. After script feedback/request updates are formed, only the effective execution clone clears a resolved None body before interpolation; unrelated unresolved variables in an unused draft therefore do not prevent connecting and no synthetic body-clearing update reaches the editor. The API still checks GET/None after pre scripts, and the engine independently rejects an active body mode or non-GET method before connecting.

A new regression first reproduced400 for the retained None draft. Both real SSE/WebSocket endpoints now assert empty received request bytes; saved draft JSON containing an unresolved template remains identical and returns no request update. For each protocol, script changes to POST or JSON return400 with no additional fixture connection. All7 protocol API integration tests passed; the prior8 protocol crate tests also passed. A direct-engine fixture verifies bad method/body rejection before any connection and successful zero-body transmission despite retained draft text. The direct-engine fixture passed (1 test;6 existing fixtures filtered); strict protocols/server all-target Clippy with no-default-features, formatting and diff whitespace checks all passed afterward.

## Independent review round2 templated-sensitive-name correction

The reviewer’s `{{header_name}}`→Authorization case first reproduced an unmasked script log in the added regression. Privacy capture now classifies both raw and bounded-resolved header/query keys while retaining explicit secret flags and raw-sensitive classification. URL query inspection covers both raw and individually resolved URLs, so a templated base URL cannot hide a known sensitive query name before a script fails. Original values and resolved values remain in the worker privacy set even after header removal.

The isolated pm runtime resolves header names/values with its existing bounded `replaceIn` against current effective variables. A common capture helper runs when mutable headers are added, read or removed, preserving intermediate credentials even if their name alias becomes sensitive after addition. Stored names/values and the public API shape are unchanged.

The exact original templated-header log/remove fixture passes, along with script-added templated key/value, updated alias, alias changed after addition, original/fresh-header exception, and templated query/URL-query exception cases. `cargo test -p moleapi-script-runtime -p moleapi-server --no-default-features`:50 passed (12 runtime,38 server), zero failures. Expanded targeted template regression passed afterward. Strict runtime/server all-target no-default Clippy, formatting and diff checks passed. The reviewer’s exact `repro-fix-round.py` exited0: direct and templated sensitive header logs `[REDACTED]`, original failed script400 `[REDACTED]`, opaque secret query `[REDACTED]`, and logout200/create401.

## Independent review round3 mutable-alias failure correction

The exact public→Authorization alias change followed by a direct literal throw first reproduced a raw credential400. Current request credentials are now captured before/after scoped set/unset/clear and URL assignments, and initially at worker bootstrap. This covers mutable header names, URL query names/values, saved query-row aliases, and auth fallback values revealed by scope removal. Headers retain the existing bounded interpolation; URL userinfo/query discovery uses mature Rust `url`/`percent-encoding` callbacks and the same bounded Rust taint collector. No public variable values are classified wholesale.

Unknown header-name aliases conservatively retain their raw values and potential Bearer/Basic credentials. Ordinary missing-variable preparation marks failure metadata uncertain without rejecting a successful pre-fill workflow. Cyclic/oversized/work-quota capture failures persist even when caught. Rust tracks capture transactions spanning scope/URL mutations; a failed or interrupted snapshot cannot report complete privacy metadata, including interruption between a mutation and its subsequent snapshot. Failed transactions also prevent a successful worker result if the script catches the failure. Capture is bounded to1000 header/query rows,1000 URL query fields,5MiB fields/URLs,16384 visits and the existing5000-value/4MiB private history, VM deadline/memory and process deadline limits.

Regressions cover the exact no-read/no-remove direct throw, sensitive→public transitions, unset/clear revealing inherited sensitive aliases, URL add/remove/alias/userinfo cases, query-row alias changes, auth fallback after environment unset, successful initially-missing-alias pre-fill and visible ordinary public header/query logs. Quota/cycle/caught/interrupted transaction tests verify incomplete metadata remains fail-closed.

Final gate: `cargo test -p moleapi-script-runtime -p moleapi-server --no-default-features`:52 passed (13 runtime,39 server), zero failures, including all9 protocol API tests. Strict runtime/server all-target no-default Clippy, formatting and diff whitespace checks passed. Exact reviewer `repro-alias-failure.py` exited0: `ALIAS_CHANGED_THEN_THROW`400 `[REDACTED]`; all earlier header/query repros remained masked; logout200/create401. Source handed back for the controller’s final independent review and serial verification. No commits, pushes, frontend edits or local Docker operations performed by this worker.
