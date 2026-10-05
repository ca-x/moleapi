# Independent Data frontend review

Authority: `docs/specs/data-client.md`; base `bb5b2ee`. Reviewed new `web/src/features/data` and its RequestEditor/controller/session/API/types/i18n/native-picker integration. No implementation/source edits, browser-process changes, Docker or subagents.

## Important findings

1. **P1 — Late query admission completion mutates the next scope.** `web/src/features/data/DataWorkbench.tsx:21–22` reserves a query, awaits `channel.send`, then unconditionally switches tabs or calls `model.release()`. The shared channel correctly returns false for an old owner/workspace/request/environment/session, but this continuation is not fenced. Data-to-Data navigation keeps the workbench mounted. If a query in the new scope is reserved before an old send resolves false, the old continuation clears its active state, hides/disables cancellation and permits another Run while the server is already executing. Capture/check the full scope and session/query ID before any post-await UI mutation; make release conditional on its reservation. Add deferred-send navigation/reconnect coverage.

2. **P1 — File-picker completion can overwrite edits or misidentify the connected dataset.** `DataSourceFields.tsx:12` retains the `change` callback from the render that opened the picker. That callback (`DataWorkbench.tsx:20`) spreads the protocol captured in that render. Editing SQL/read-only while the dialog or file read is pending therefore gets overwritten when the picker resolves. The commit guard checks only mounted/source, allowing completion after Connect started; the UI then displays the new file while the active session still contains the previously captured file bytes. Its remount key at line28 also omits environment, despite the spec's scope fences. Read the latest protocol at commit, capture a full picker scope/generation, invalidate on connection/environment transition, and either block Connect while selection is pending or reject a late commit after connection begins. Cover edited SQL preservation and deferred pick plus Connect/scope change.

3. **P2 — Stop/disconnect leaves a query permanently marked as running.** `useDataEvents.ts:14–23` handles Data result events but ignores terminal `state` events, and only resets when the session ID changes. Explicit session close keeps the same session ID. The backend deliberately skips `data_cancelled` when the session cancellation token is cancelled, so closing a running query leaves `active` set and the result `done:false`; the view keeps its querying badge and an enabled Cancel button that cannot send to the closed session. Reconcile terminal session state after the final event batch, retire the active reservation and explicitly mark/withhold incomplete results without claiming rollback or successful completion. Test close/transport-error while a query is active.

4. **P2 — The existing Run keyboard shortcut is a no-op after Data connects.** Controller `sendRequest` at `useWorkbenchController.ts:69–80` routes Data exclusively to `protocolSession.connect`; the global Ctrl/Cmd+Enter handler invokes that function. `connect` returns immediately for an already open session, so the shortcut never runs the current statement/selection, even inside the SQL editor. Register a Data run handler like MCP/GraphQL, dispatch connect before connection and selected-query Run afterward, and exercise the actual keyboard path.

These are static call-path/React-lifetime findings; no browser reproduction is claimed. Existing tests cover selected SQL, exact cell text/export, schema completion, event gaps and some stale events, but not these continuation/picker/terminal/keyboard paths.

## Fresh verification and limits

- `npm --prefix web test -- --reporter=dot`: 40 files / **137 tests passed**, exit0.
- `npm --prefix web run typecheck`: exit0.
- Root owns current embedded browser QA and final build verification; neither was replaced or restarted.

Exact integer/decimal sorting uses Decimal comparisons; typed JSON export preserves duplicate column names, null/binary identities and exact numeric text. CSV uses mature quoting/formula escaping and explicitly exports all returned rows. Completion preserves database namespaces. Shared protocol polling fences account/workspace/request/environment/session updates. The findings concern additional Data-local asynchronous continuations and terminal-state integration.

No native-platform or complete Data parity approval is inferred. Review-directed fixes and focused regression evidence are needed before frontend approval.

## Focused fix re-review

**Approved for all four review findings.** No definite important issue remains within this fix scope.

- Query admission completion checks authenticated/account/workspace/request/environment/session identity and mounted state. Release is conditional on its own query ID; synchronous in-flight reservation rejects repeated dispatch before React renders.
- Picker completion uses the latest source/callback/connected state; it preserves intervening SQL edits and rejects committing into a connecting/open session. Environment changes remount the scoped picker and invalidate its former mounted guard.
- Terminal closed/error events clear reservations and withhold unfinished results, while a prior completed result remains intact. The UI does not invent successful completion or rollback.
- Data registers its own current run dispatcher in the shared controller path, connecting when closed and running selected SQL when open.

Fresh independent verification: frontend **144 tests passed across 42 files**, exit0; typecheck exit0. Added deferred old-send/new-scope, picker edit/connect transition, terminal-close and dispatcher open/closed/repeated-admission tests exercise the corrected paths. Final embedded build/browser/runtime gates remain root-owned; native/full-parity approval is not implied.

### Editor-focused keyboard follow-up

Root browser QA found that CodeMirror's default Mod-Enter inserts a blank line and prevents the global shortcut before the dispatcher is reached. The follow-up `DataSqlEditor` change was inspected independently: `Prec.highest` gives the query binding priority, its handler calls `latest.current.run` and returns true, and Workbench forwards the current dispatcher through a ref. This prevents a stale callback and causes the existing window handler's `defaultPrevented` check to suppress duplicate dispatch. Synchronous query admission also rejects repeated events before React renders. No definite important issue found in this narrow change. This paragraph records static inspection only; successful editor-focused browser execution is not claimed and remains root-owned verification.
