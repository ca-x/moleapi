# Shared HTTP request editor slice

**Parent plan:** `tasks/implementation/workspace-collaboration-plan.md`. **Spec:** `docs/specs/workspace-collaboration.md`. This advances the hosted member workbench; it does not complete protocol-specific, scenario or native hosted editing.

## Contract

- For a saved HTTP request in a shared workspace, editor/admin can edit query and header rows, body kind and body (including the existing binary/multipart editor), authentication, assertions, extractions, scripts and request network settings through the same pure components used by the owned request editor. The workspace's owner/revision CAS PUT remains the only persistence path.
- Viewer sees the safe read-only request projection and no edit or execution controls. Non-HTTP protocols keep the existing limited shared view until their dedicated editors receive an explicit member policy.
- A dirty draft disables canonical saved-request execution. Editing any request setting clears prior send/run/Mock results and fences delayed responses. Switching body kind uses the shared body-mode conversion so binary/multipart descriptors cannot be sent as text accidentally. Unsupported or invalid request configuration returns existing server validation errors on save; no silent truncation or format change.
- The shared view reuses the app's dark setting and supports 390 px English/Chinese keyboard and touch layout, including horizontally accessible editor tabs.
- OAuth1/OAuth2 token acquisition, reveal and deletion stay in the owner's token vault. The shared view may edit configuration but does not mount the owned editor's token managers or bind member-owned token IDs to owner execution.

## Acceptance

- Frontend tests cover query/header/body/auth/assertion/script/network edits in the canonical CAS payload, binary/multipart mode switching, dirty execution guard, viewer denial, request selection isolation, concurrent revision conflict and both OAuth token-manager exclusions.
- Existing server collaboration tests cover member canonical writes and saved-request execution; extend one to prove edited HTTP query/header/body reaches a local wire fixture and history under the real member actor.
- Real 390 px bilingual browser save/reload/send flow, full frontend/Rust tests, strict Clippy, formatting, production frontend/embedded server builds, independent review, fixed-head CI and package gates before any parity claim.
