# Shared HTTP draft execution slice

**Parent:** `tasks/implementation/workspace-collaboration-plan.md`. **Spec:** `docs/specs/workspace-collaboration.md`. This opens one additional member execution path for finite HTTP requests; protocol sessions, public Mock and runner work remain separate.

## Contract

- Editor/admin may POST a complete HTTP request draft to an explicit owner/workspace route, with canonical collection ID and expected workspace revision. The draft may modify a saved request or have a new request ID in an existing collection. It is never persisted implicitly. Viewer, removed member, wrong owner/workspace, stale revision, absent collection and non-HTTP protocol reject before outbound work.
- Execution uses the canonical owner's workspace data, environment, collection ancestry, variables and network policy. The submitted request spec is validated with the existing request validator. An ID already saved in another collection is rejected to prevent an unintended parent context. The real actor is audited as `request.draft.execute`; owner-scoped bare-ID routes remain closed to members.
- Membership generation/role and canonical workspace revision are checked during long execution and once more before returning private data. A concurrent role change/removal or revision change withholds results. Outbound side effects already sent cannot be undone; generated history remains owner-scoped and redacted using existing rules.
- The shared HTTP editor can create a new unsaved request in a canonical collection and run the current draft. A dirty draft shows a distinct draft-send action; the existing saved-send action still runs the canonical saved version only. Selection, edits, save, role/account change and revision conflict fence delayed UI responses.

## Acceptance

- Real local HTTP wire: a member modifies a saved request and executes unsaved bytes without changing the owner workspace; a new unsaved request executes with collection variables/auth; history and real actor audit are present. Viewer, stale revision, cross-collection ID collision, non-HTTP and cross-instance role downgrade deny/withhold with no private result.
- Frontend: draft-send body contains request, collection ID and expected saved revision; saved-send remains unavailable while dirty; a new draft request can run and then be saved via CAS; stale result isolation holds across request/version/account changes.
- Real 390 px English/Chinese browser flow, full Rust/frontend tests, strict Clippy/formatting, production builds, independent review, fixed-head three-database CI and Docker/native gates before a broader member execution claim.
