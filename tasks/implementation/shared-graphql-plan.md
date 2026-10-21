# Shared GraphQL workbench plan

> Use executing-plans and verification-before-completion.

**Goal:** Members create/edit/save GraphQL requests and use queries/mutations, variables, Schema introspection/association and saved subscriptions with member fences.
**Spec:** docs/specs/workspace-collaboration.md, full feature matrix remains active.
**Architecture:** Extract the existing controlled GraphiQL IDE, preserve owned workbench behavior, parameterize the Rust fetcher transport for member execute/session routes. Shared Schema routes reuse canonical owner execution without local overrides; stage returned schema/reference and explicitly CAS Save.

## Constraints and review focus

- Shared finite drafts accept HTTP and GraphQL query/mutation only; subscription creation uses saved ID/revision, never an unvalidated request override.
- Reuse exact editor text, operation selection and variables; no editor/browser storage persistence, no owner endpoints or member-local overrides.
- Saved subscription admission must agree with saved query/variables/operation; edits require Save and close/suppress old sessions.
- Role/revision/credential final fences, content-free actor audit, creator session/cancel and losing watcher cleanup.
- Stage introspection spec/reference only while canonical context and request snapshot remain current; no automatic canonical write or stale private result release.

## Task 1

- [x] RED real member query/mutation/introspection/schema/subscription tests; implement member routes and whitelist rules.
- [x] Full request/header/auth/script/source preservation, stale/viewer/foreign/creator/role/version/logout and draft subscription denial coverage.

## Task 2

- [x] Extract controlled GraphQL IDE and extend fetcher transport with owned compatibility tests.
- [x] Shared request creation/configuration/Schema and subscription UI with scope/selection/edit/reset tests.

## Task 3

- [x] Full frontend/server gates, production/Clippy/format/embedded build.
- [x] Independent review, bilingual390 real query/mutation/schema/subscription/cancel acceptance.
- [ ] Docs/commit/push/PR and CI observation.

## Verification ledger

- Backend original whitelist RED: new real GraphQL member test returned400 HTTP-only. Implemented finite GraphQL and canonical schema/member saved subscription policy; new integration test passes with role/logout session cleanup.
- Disposed editor callback RED: callback from the old mounted scope changed the new account draft. Guarded mounted/current/conflict callbacks; regression passes.
- Full frontend:126 files,653 tests pass. Backend library22 + collaboration13 + shared GraphQL1 + shared Mock4 + owned GraphQL16 =56 pass.
- Production43 chunks without static cycles; strict server Clippy, package format and embedded server build pass. CSS-only mobile correction subsequently rebuilt production and embedded server.
- Independent final source review found no reproducible issue; reviewer declined runtime Monaco/WS and full gates, which are covered by the actual browser/gates here.
- Real Chinese390: canonical query returns Hello member; introspection candidate leaves server revision1/specification count0 until explicit Save, then saved linked schema loads. Actual edited multi-operation AST selection Double with value9 returns18 and CAS retains variables and operation. Saved waiting subscription emits1; Stop closes member session; edited unsaved subscription yields Save-first error. New GraphQL request executes before Save then persists.
- Browser viewport initially exposed overly narrow side-by-side query/results; scoped mobile CSS stacks editor and result, with a minimum variable editor height. Final bilingual artifact verification passed on the rebuilt embedded server.
- Rendered scroll regression: upstream `.monaco-editor` is absolute and its shared host was static. After scrolling, the query section top257.5 differed from text top1409.5; adding relative positioning to the shared editor host restores the text to its section. This is scoped to the shared GraphQL host.
- Final rebuilt browser: English/Chinese390 returns18 and query text remains inside its section after scrolling (section257.5/text273.5 or section285.5/text301.5); width390, browser errors empty. Desktop1280 remains row layout and fits. Screenshots retained in docs/images/workspace-collaboration-graphql-{en,zh}-390.png.
- Whole395-capability objective remains active; this completes only shared GraphQL parity. Current-head CI/native/release gates remain separate.
