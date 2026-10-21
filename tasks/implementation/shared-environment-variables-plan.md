# Shared environment variables slice

**Parent:** `tasks/implementation/workspace-collaboration-plan.md`. **Authorities:** `docs/specs/workspace-collaboration.md`, `variables-001/003/004/005` and `collaboration-010` in `docs/FEATURE-MATRIX.md`.

## Contract

- The shared workbench exposes project global, collection and environment variable rows from the canonical owner workspace. Viewers see only the existing privacy-projected values and cannot edit them. Editors/admins can create, rename, select and delete environments and edit all three shared variable scopes through the existing owner/workspace revision CAS. Deletion is confirmed and remains a draft until Save.
- Reuse the existing `PairEditor` labels, enabled and secret controls. Show shared values only. Do not mount `VariableScopeEditor` or owned variable import/export: their local override and file routes use the member's unrelated owned-workspace context. Local values are neither displayed nor copied into the shared draft.
- A remote revision conflict uses the shared view's existing discard/keep flow; pending environment deletion confirmation resets on canonical revision change. A viewer must never receive an owner's secret variable value in the API or DOM. Invalid variable data is rejected by the existing workspace validator without changing the canonical revision.
- Keep owner-only variable file import/export, personal local overrides and team-level variables outside this slice. This adds workspace-shared editing, not a new team variable scope.

## Acceptance

- Hosted integration: editor CAS writes an environment and project/collection variables visible to the owner and usable by canonical shared execution; viewer projection omits secret values and viewer PUT is denied. Stale revision and invalid values leave the owner workspace unchanged.
- Frontend: editor create/select/rename/delete confirmation, project/collection/environment secret rows, one Save path with the expected revision; viewer has read-only projected rows and no mutation controls. Cross-account/workspace switching drops drafts and pending confirmations.
- Run affected frontend/backend tests, strict checks and production/embedded builds; verify a real 390 px English/Chinese member flow and independent review before claiming this slice's evidence. Fixed-head three-database CI and installed native packages remain separate gates.

## Steps

1. Add frontend regressions for viewer read-only projection, editor variable edits and environment management, stale confirmation and CAS payload; add hosted projection/CAS assertions where existing collaboration fixtures can cover them.
2. Implement a focused shared variables panel using the existing canonical `WorkspaceData`, `PairEditor` and shared view Save/conflict boundary. Wire a Variables tab, with confirmation tied to canonical revision.
3. Run the acceptance gates, record exact evidence and limits in `docs/COLLABORATION-COVERAGE.md`, then push the verified draft PR head.
