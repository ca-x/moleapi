# Shared Mock route debugger implementation plan

> **For agentic workers:** Execute inline with executing-plans and verification-before-completion; one independent final source review.

**Goal:** Shared editors/admins test saved Mock routing, matching and parameterized dataset bindings from the workbench with explicit temporary incoming HTTP fields and inspect actual responses without publishing or saving those fields.
**Architecture:** Add a canonical revision-bound member JSON preview route that builds a synthetic incoming request, reuses the existing in-process Mock router, and returns a bounded response envelope. Outer application credentials stay separate from simulated request data. Extend the shared Mock panel with controlled temporary request inputs, guarded dispatch/results and refreshed call metadata.
**Tech Stack:** Existing Axum/URL/percent-encoding, Mock matcher/scripts/data engine, serde, React/Radix/CodeMirror/ResponsePane. No network self-request or new wire parser/dependency.
**Spec:** docs/specs/mock-request-matching.md, mock-data-binding.md, mock-call-logs.md and shared routed debugger subsection in workspace-collaboration.md.

## Constraints/interfaces

- POST /owners/{owner}/workspaces/{workspace}/mock/route-preview takes expected_revision, method, path, query/header Pair arrays and UTF8 body only; deny unknown owner/workspace/request/locals/credential overrides. Accept HTTP methods GET/POST/PUT/PATCH/DELETE/HEAD/OPTIONS, absolute path within this Mock namespace,100 query/header rows, bounded URI/header/body budgets. Query/header credential-like or hop fields follow the existing safe incoming contract and are rejected visibly at the debugger boundary rather than silently simulated. No browser Authorization/Cookie or ambient credentials enter the synthetic request.
- Canonical owner workspace, role/editor, exact revision and actor credentials admit the operation. Record mock.route.preview actor/action only, never method/path/query/header/body/response. Watch credential/member/revision throughout dispatch and bounded response read, then final auth→member/revision release fence. Viewer/foreign/stale access fails before matcher/scripts/data.
- Reuse existing matcher specificity/priority/body/query/header rules, selected-example scripts/data/traffic limits, outcome/privacy-only durable logs and current canonical source. No HTTP request to any supplied host/path; path is a local match target, never an app API dispatch. Upstream Mock401/404/429/500 stay in the HTTP200 envelope; outer authorization failures remain real application4xx. Response headers/body are inert data, with byte/status/timing/bounds preserved. No Set-Cookie/navigation/content-type side effects in the browser.
- Optional internal selection metadata identifies actual matched canonical request/example and outcome without changing existing wildcard/direct response bytes. Unmatched/runtime failure remains inspectable; required log-write failures never look like successful Mock responses. Existing ANY member route and direct example preview remain compatible.
- UI fields are session-local, seeded from the selected saved HTTP route without writes. Expose concrete path, method, repeated query/header pairs, body and explicit Test; parameterized binding can now be exercised directly. Saved clean revision required. Pending/late controls/results bind account/token/role/workspace/cache/revision/selection/draft/input/epoch; dirty or stale state discards output and cannot issue a new call. Runtime data/response never become canonical examples without a separately authorized explicit edit.
- Response uses existing mature inspector, matched IDs resolved only from current canonical data, logs refreshed after current completion. Viewer has no debugger controls. Owned/public shares and DELETE logs remain owner-only; no new public delegation.

## Task1: backend

Files: collaboration/mock_preview.rs, collaboration.rs, mock.rs, lib.rs, tests/collaboration_mock_preview.rs.
- [x] RED missing route and outer role/revision/foreign/malformed budget guards.
- [x] Build synthetic URI/headers/body with mature existing utilities; canonical admission/watch/final fence; response envelope and internal selection metadata, preserving router/log/traffic/script semantics.
- [x] Real router fixtures exercise method/path/query/repeated pairs/header/body/specificity/priority, actual parameterized dataset and script owner-collision, real statuses/body/headers, outer token exclusion, traffic/rate/outcome logs, no canonical writes and role/revision/logout cancellation. Route/header/path budget negatives do not run scripts/data.

## Task2: frontend

Files: SharedMockRouteDebugger.tsx/tests, SharedWorkspaceView.tsx/tests, i18n/styles.
- [x] RED shared parameterized example test flow, local temporary fields, actual matched result and no workspace PUT/owned resource path.
- [x] Controlled method/path/query/header/body inputs and member envelope adapter; mature ResponsePane plus actual match identification and explicit bounds/help.
- [x] Delayed request/data/input/cache/account/credential/revision/reopen/mounted replies/retained callbacks cannot update another context; failure releases busy for explicit retry, target401 does not trigger application logout.

## Task3: verify/deliver

- [x] Full frontend/typecheck/production and affected backend, strict scoped Clippy/fmt/diff and built server.
- [x] Independent final review; actual en/zh390/1280 path/query/header/body/dataset/rule/401/404/traffic/inert-content/log/reload/current-scope flows and a11y/screenshots; preserve exact canonical data.
- [ ] Coverage/map/status/ledger, commit/push/preserved PR and exact-head CI/Docker. Full28-module395-capability/native sync/distribution/platform and EDAS scope stays open.

## Review focus

- Application credentials/transport headers must never reach Mock scripts or dataset parameters; simulated fields do not replace outer authentication.
- Decoding/normalizing path or repeated query input must match the existing wildcard semantics and never address another app endpoint or perform a network request.
- Mock401/404/error headers/body cannot log out the app, set cookies, navigate or execute HTML; actual outer401 still revokes the current session.
- Late result/match labels and current call-log refresh must not cross current account/request/cache/revision/input boundaries or falsely imply a canonical write.
- Revocation while matching/delaying/running script/data/collecting body must drop the owned operation and withhold its response; log privacy and failure behavior remain intact.

## Ledger

- Previous goal turn was progress: shared Dubbo39379cf committed/pushed, current worktree clean. Exact Docker39514953946 and Application39514953916 confirmed live; source/worker passed; no timeout-driven restart.
- Remaining gap identified from actual UI/spec: parameterized Mock bindings disable direct example preview and only show a wildcard route string, leaving no workbench route test controls. Public sharing/clear-log policy remains explicitly owner-only and is not expanded here.
- Ruling: use a member JSON response envelope around existing in-process matcher instead of issuing browser wildcard requests — separates application credentials from simulated inputs and preserves Mock401/Set-Cookie as inert response data — cost if wrong: one member endpoint and local DTO adapter, preserving original wildcard API.

- Backend checkpoint: existing member wildcard logout fence was observed returning201 after logout (RED); shared revision dispatcher and authenticated watch/final gate now pass11/11 affected Mock tests, plus exact route inventory1/1. Full owned/public/native/log compatibility suite running.
- Frontend checkpoint: route debugger mounted with temporary method/path/repeated query/header/body fields and mature ResponsePane. Component9/9, targeted workbench3/3 and TypeScript pass. Full frontend gates/reviewer/browser/delivery still pending. Existing test selectors scoped to response-header editor as new query/header editors share an Add-row action.
- Independent review found URL dot-segment normalization could execute a different route. Regression raw wildcard404 vs preview201 observed RED. Fixed by percent-encoding the original path without URL normalization, keeping URL only for query encoding. Literal/encoded dot segments and escaped path/query script-snapshot tests GREEN; same independent reviewer confirmed resolved with no further findings. New affected suites12/12 pass.
- Full backend compatibility checkpoint79/79 pass before added path regression. Initial frontend full771 pass/1 missing translation; translation added and catalog2/2 pass; final full run pending. Build inputs race interrupted initial Clippy when frontend output changed; successful fixed-input Clippy followed, final gates now explicitly sequential frontend→Clippy→server to avoid embedding inconsistent assets.

- Final local gates: full backend80/80, full frontend142 files/772 tests, final focused11/11, type/build38 chunks, sequential strict Clippy+embedded server, independent review/recheck and actual bilingual390/1280 browser/data/status/log/a11y/reload/canonical preservation pass. Commit/push/PR/exact-head CI follows; whole28-module395-capability objective remains active.
