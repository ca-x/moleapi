# Shared Data client implementation plan

> Use executing-plans and verification-before-completion.

**Goal:** Members can create/edit/save Data requests and use canonical saved Data sessions with SQL/schema/results/cancel under member boundaries.
**Spec:** docs/specs/workspace-collaboration.md; full 28-module matrix remains active.
**Architecture:** Controlled request editor reuses DataSourceFields/SQL editor. Shared live panel adds Data query controls/events/results; shared backend delegates existing scripted Data send. Carry member/credential checks into query admission and completion.

## Constraints and review focus

- No owner routes or copied workspace; saved ID/revision admission, canonical owner, no viewer controls/private results.
- Preserve scripts/credential privacy; query preprocessing and completion fence live actor, credential and canonical revision.
- File imports fence exact source/config identity, scope/revision/conflict, pending unmounts and source changes.
- Query/schema/cancel messages only for Data; existing SSE/WebSocket policy unchanged.
- Active connection closes on draft edits/tab/selection/scope changes; late operations cannot retarget.

## Task 1

- [x] Failing tests for Data creation/editor/member session and query flow.
- [x] Data request editor, scoped file admission, Data session UI and backend.
- [x] Frontend/server tests and typecheck, bilingual copy.

## Task 2

- [x] Production build, strict Clippy, formatting, embedded rebuild.
- [x] Independent review and real bilingual 390px member acceptance.
- [x] Coverage/docs/commit/push/CI.

## Evidence ledger

Task 1: controls RED missing Add Data; shared backend RED 400 when original protocol whitelist restored. Source fields existing regression first failed after overbroad identity, fixed to preserve SQL while fencing actual source fields. Existing linked Schema revision test failed 201 vs 200 from pre-notification race, fixed with canonical cache admission. GREEN frontend610/125, targeted145, backend43.
Final review: shared_report_export_review identified Important post-check history insertion race, no second concrete issue.
Final: fixed history race — member_completion_waiting_for_database_writer_cannot_insert_after_demotion RED nonempty stale history → GREEN empty history. Shared canonical workspace row lock + membership/revision admission + history insert now share one transaction. Backend suite43/43. No re-review required by execution workflow.
Task 2: production45 chunks, strict Clippy, package format/diff and embedded rebuild passed. Runtime English/Chinese390: real browser file input read, canonical Save/connect/3 rows, schema/search/sort, parsed JSON export, long query cancellation, source/revision unchanged by temporary SQL, no overflow/errors.
Final: Ruling: browser headless system chooser was suppressed while agent-browser uploaded to the actual file input — production file.arrayBuffer/base64/validation callback and CAS path ran, native dialog itself remains in installed desktop acceptance — cost if wrong: native-only file chooser integration can escape this pass.
