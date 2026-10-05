# Independent HTTP body frontend review

Reviewed the unstaged frontend HTTP body slice against base `daabcb0` and `docs/specs/http-request-bodies.md`: structured Zod source, body editor, shared binary picker/Data wrapper, RequestEditor/Workbench scope and bilingual labels. No source edits, delegation, Docker or root QA-process changes.

## Important findings

1. **P2 — Content-Type transition only handles multipart.** `web/src/features/requests/RequestEditor.tsx:413` disables stored Content-Type rows only when `body_kind === "multipart"`. Switching JSON to binary keeps an enabled `application/json` header; actual transport honors that explicit header instead of the selected file MIME (`crates/core/src/transport.rs:77–80`). This violates the spec's “UI mode changes disable stored Content-Type headers with visible explanation and preserve their values,” and produces a visibly selected binary file with stale JSON wire metadata. Centralize the mode-transition policy, preserve header values but disable incompatible stored Content-Type rows for the defined transitions, and provide explicit visible feedback that they were disabled. Cover JSON/header to binary and multipart transitions.

2. **P2 — Deferred file reads overwrite an explicit MIME edit.** `web/src/features/request-body/RequestBodyEditor.tsx:18–19` correctly reparses the latest source, but then replaces its MIME with `file.mime || current.mime`. Browser files commonly have a nonempty detected type. Selecting a file, changing its MIME field while reading is pending, and resolving the picker therefore erases the user's deliberate MIME value. Both binary and multipart file paths do this, despite the edit-preservation contract. Capture the starting target MIME and preserve a changed current MIME; infer browser MIME only when there is no intervening explicit edit. Add deferred-picker coverage for both modes, preserving unrelated fields and repeated part names.

Static findings; browser reproduction is not claimed. These new body components have no focused tests at this reviewed snapshot. Existing Data picker tests cover their prior wrapper behavior but do not cover the new body's asynchronous MIME commit or mode-transition policy.

## Boundaries checked

- Editor remount scope contains account/workspace/request/environment/body kind; busy rising edges invalidate pending selection tickets, including a send that completes before the picker returns.
- Picker success reparses latest source and looks up multipart rows by stable ID, preserving latest text/name edits and discarding removed/file-to-text targets. Shared browser/native readers bound file bytes to5MiB and close native handles; paths are selected explicitly.
- Base64 is not rendered into CodeMirror during normal structured editing. The source editor mounts only when explicitly expanded, and malformed JSON falls back to an editable source view.
- Multipart rows preserve order and allow repeated names; enabled state and selected-file null versus empty bytes are represented distinctly. Backend separately validates unique IDs and aggregate limits.
- Actual request bytes/redirect behavior and export/privacy checks remain backend/root-owned, not inferred from this UI review.

## Fresh verification

- Frontend suite: **144 passed across42 files**, exit0.
- `npm --prefix web run typecheck`: exit0.
- Root owns final build/browser/native-shared QA. No platform or parent-parity approval is implied.

Fix the two findings and add focused regression evidence before frontend approval.

## Focused fix re-review

The MIME overwrite finding is resolved for both binary and multipart paths: picker admission captures the target MIME, and completion preserves a current MIME that differs from that starting value. The binary deferred-picker regression passes. Busy-start invalidation is also covered through a busy true/false cycle before completion.

Content-Type transition behavior is now centralized in `bodyModePatch`; entering binary or multipart disables stored literal Content-Type rows while preserving their values. The JSON-to-binary regression passes. One part of finding1 is still pending at this reviewed snapshot: the spec's visible explanation that stored headers were disabled. Binary mode has no Content-Type notice; multipart's existing notice tells users to disable a manual header rather than explaining the automatic transition. Root was notified to add a concise bilingual explanation before approval.

Fresh independent focused check: `npm --prefix web test -- src/features/request-body --reporter=dot` — **3 tests passed across2 files**, exit0. Actual upload browser QA and full final source gates remain root-owned. No source edits by this reviewer.

### Final scoped approval

**Both findings are resolved; scoped frontend review approved.** The shared file-body editor now shows an explicit notice in English and Chinese explaining that file-mode transitions disable stored Content-Type headers and preserve their values. The Choose/Add buttons use the existing gray theme; no asynchronous or representation behavior changed in that color adjustment. Fresh independent focused tests again passed **3/3 across2 files**, exit0. Root-reported actual binary/repeated-field multipart browser echo results were noted but not independently reproduced by this reviewer; final rebuilt contrast/a11y gates remain root-owned, and native dialog coverage is not claimed.
