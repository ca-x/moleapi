# Shared saved Mock editor slice

**Parent plan:** `tasks/implementation/workspace-collaboration-plan.md`. **Spec:** `docs/specs/workspace-collaboration.md`. This is one member-workbench slice, not completion of Mock or collaboration parity.

## Contract

- An editor/admin can add and remove a saved response example and edit its name, HTTP status (200–599), body, response headers and delay (0–30000 ms) in the canonical shared workspace draft. A viewer receives only the existing safe projection and no editing controls.
- The existing owner/workspace CAS PUT saves these edits; preview always runs the last saved version. Dirty drafts disable preview. On a concurrent owner/member change, show the conflict before any new save or preview. Changing selection or workspace version clears prior preview output.
- Preserve advanced Mock fields in untouched examples; editing a basic field must not erase schema, data, script, matching or fault settings. Keep public share management owner-only.
- Shared example editing is hosted-only. Native hosted sync and complete owned editor parity remain separate work.

## Acceptance

- Frontend regression: editor add/edit/save/preview uses the canonical owner route and saved example ID; viewer cannot edit; a dirty draft cannot preview; conflict recovery keeps the selected member draft or loads the server version without stale preview.
- Server integration: the shared CAS PUT already exercises editor authorization and validates example fields; extend the collaboration test to prove an editor's changed saved example is served to the owner and member, with actor audit and revision checks intact.
- Real 390 px bilingual Chromium flow; full frontend suite/build, collaboration/database tests, strict Clippy, formatting, embedded server build, fixed-head CI and independent review before expanding the draft PR claim.
