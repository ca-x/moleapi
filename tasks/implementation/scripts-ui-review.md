# Scripts and local-variable UI scoped review

Status: scoped re-review complete against `8908d2123f74236666b838923919b62bf30bb9d3` plus the current uncommitted extension. Three material frontend defects were reproduced and fixed by root during review. No material finding remains in the requested scope after the focused fixes and checks below. This reviewer edited only this report and temporary diagnostic files, not implementation code.

## Scope and evidence boundary

Reviewed `features/scripts`, `features/variables`, `useRequests`, `useRunner`, workspace close guarding, shared transport types/editor, their integration points, and `desktop/capabilities/default.json`. Depth was deep within this requested surface. Backend implementation review was excluded; response declarations were read only to check additive frontend transport fields.

Root's concurrent formatting, canonical dialog permission change, and all three focused fixes were re-read. HEAD remained `8908d2123f74236666b838923919b62bf30bb9d3`. This is a current-tree scoped review, not an isolated release-readiness or native-platform verification verdict.

## Findings and verified resolutions

| Finding | Original concrete behavior | Root fix and current evidence | Status |
| --- | --- | --- | --- |
| P1: local-storage write failure crashed rendering | `write` and `reconcile` called `localStorage.setItem` inside React state updaters without a catch. A store throwing `QuotaExceededError` made `act(write(...))` throw and abort the hook/component render. Typing, unlinking or successful script updates could hit this path and strand an unsaved workbench. | Persistence now happens outside the React updater, catches failure, preserves the prior rendered/local state and reports a toast. Failed rename/delete reconciliation returns false so its shared row change is not acknowledged. The repository quota regression and external actual-hook test both pass; no private value falls back to shared data. | Resolved |
| P2: rename through empty key lost the browser override | For the same row ID, `token -> '' -> renamed_token` deleted the old bucket on the first keystroke and had nothing to move on the second. A one-event `fill` replacement worked, masking the ordinary clear-then-type case. | `editing:<scope>` buckets retain values by row ID during a blank-name edit; VariableScopeEditor passes IDs; the buckets are not emitted as execution locals. Naming restores the value, and actual row deletion removes pending values. Repository regression passes. External simultaneous two-row blank/restore check preserves A and B independently and emits no pending overrides. | Resolved |
| P2: native blank rows ignored row identity | Two native rows `{id:a,key:'',local_value:A}` / `{id:b,key:'',local_value:B}` were both addressed by key `''`. Reading row B returned A; editing row A patched both `local_value` fields. | Native read/write use row ID when supplied; key-only script updates retain key lookup. A stale missing UI row ID does not create a replacement. The native-mode repository hook test and external native-branch test now read B correctly and patch only A, preserving B and shared values. | Resolved |

Original locations were `useLocalVariables`' persistence/native branches and `localValues.reconcileLocalRows`. The current implementation is under `web/src/features/variables/useLocalVariables.ts` and `localValues.ts`; line numbers moved during root's formatting and fixes.

The initial external two-case reproduction explicitly asserted the bad behavior and passed on the old source; it was diagnostic proof, not an intended-behavior regression. The later external suite was rewritten to assert correct behavior and now passes all four focused cases. No temporary review test was added to the project.

## Requested boundary checks

- Browser local-cache keys include encoded account and workspace identities. Hosted writes modify local bucket state/storage, not `draft.data`. Execution payloads use selected project/collection/environment `locals`; explicit empty-string overrides survive and disabled declared variables are omitted.
- Native values use `Pair.local_value`, retain the shared `value`, and preserve other pair fields. Stable row identity now works during simultaneous blank-name edits. Script-created variables can still be addressed by key without a UI row ID.
- Request and runner state retains account/workspace/request or collection identity. Both controllers check current owner/workspace refs before applying returned mutations; `apply()` repeats that check. The existing delayed-workspace regression passes. No cross-account/workspace mutation path was found in this scope.
- Returned runner results and their mutation arrays are applied in order. An external test exercised the actual `useRunner` and local-variable hook with `token=first`, `other=keep`, `unset(token)`, `token=last`. Final cache contains `token=last` and `other=keep`; the shared workspace updater is not called. Reading the latest persisted bucket before each synchronous write avoids losing preceding batched updates.
- Optional script fields, logs, variable updates and request updates match the additive transport shape. The direct response console renders request changes as plain text, identifies them as belonging to that execution, and does not patch the saved request. Request JavaScript uses mature CodeMirror language support and labels its actual content textbox.

## Native close guard and permissions

The guard reads a stable ref-backed `hasChanges` callback, prevents a dirty native close before awaiting the existing dialog, suppresses duplicate prompts, and explicitly calls `destroy()` only after confirmation. Cancel keeps default closing prevented. Async registration/disposal and listener cleanup are handled; setup failure reports a toast.

`desktop/src/main.rs` installs the dialog plugin. The current capability grants canonical `dialog:allow-message` and `core:window:allow-destroy` to window `main`. Installed SDK `ask()` invokes `plugin:dialog|message`; installed `onCloseRequested()` awaits the callback and automatically destroys only when default was not prevented. The configured permission and TypeScript flow match these calls.

The earlier `dialog:allow-ask` spelling was not a locked-version runtime defect. The downloaded locked Rust plugin `tauri-plugin-dialog` 2.8.1 explicitly defines `allow-ask` as a deprecated alias granting `message`. Root's `allow-message` replacement is valid and avoids the deprecated alias. The generated desktop schema was absent locally; no interactive native-window or packaging proof is claimed here. The native variable regression runs the actual hook branch with its native flag mocked, not an operating-system dialog.

## Fresh verification

- `npm --prefix web run test`: **26 tests pass** in five files after all focused fixes.
- `npm --prefix web run typecheck`: passes after the fixes.
- `npm exec -- vitest run --config /tmp/moleapi-scripts-ui-review.vitest.mjs`: **four external scoped checks pass**: quota retention, simultaneous hosted blank-name identity/non-execution, ordered actual runner updates, and native blank-row identity.
- Production build and root agent-browser/CI reports are context; this reviewer did not re-run a build, mutate a browser workspace, review backend fixes, launch a native desktop, commit or publish.

Final reviewed source fingerprints:

- `useLocalVariables.ts`: SHA256 `c3ef9badf85605a6553c771ad5036c9c843c4eb5cdedec06b59e1a406846466e`
- `localValues.ts`: SHA256 `968a2ca6c71a9f2e2237955aaac770ef4fee6520bf05caf800dc1c5ba35dc2c3`

Completion ledger:

1. Requested frontend contract and native-capability review: done.
2. Three material defects reproduced, delivered to root, and scoped fixes rechecked: done; no unresolved finding in the requested scope.
3. Review artifact: done, `tasks/implementation/scripts-ui-review.md`.
4. Actual native-platform confirmation and release/build integration: remaining with root; outside this scoped read-only sign-off.
