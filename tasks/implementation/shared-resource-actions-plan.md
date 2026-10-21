# Shared request and collection resource actions

Authority: `docs/specs/workspace-collaboration.md`, member resource editing and canonical workspace CAS.

1. Add pure request duplicate/move/delete and collection-subtree delete operations. Preserve request contents and IDs on move; duplicate uses a fresh ID. Reuse collection tree traversal/removal and move documents from deleted folders to the root. Keep unrelated source resources intact.
2. Detect structural dependencies: scenario roots/steps and query-backed datasets block deletion; request moves cannot leave a referencing scenario's subtree. Show named dependency IDs rather than silently deleting other resources. Reference editing remains part of the overall resource parity work.
3. Add editor/admin controls and reviewable dialogs with exact target names/counts; confirm only stages local changes. Viewers have no actions. Account/role/credential/selection/saved/conflict/data changes dismiss pending confirmations; recheck dependencies on confirm. Existing shared Save supplies audited CAS and server authority.
4. Tests first fail for missing operations/controls. Cover copy content/fresh ID, move ID/settings preservation and scenario scope, collection descendants/doc relocation, dependency retention, Cancel/confirm/CAS, viewer absence and stale confirmation fences. Add real server coverage of member deletion, viewer/stale rejection and automatic public Mock revocation without adding public-share authority.
5. Run focused/full frontend, relevant server suites, production frontend then strict server Clippy then embedded build sequentially, formatting/diff checks, independent source review and actual Chinese/English 390 px copy/move/delete/Cancel/save acceptance before commit/push PR41.

Remaining: reference-management editors, other protocols/resources, native hosted sync and current-head release gates remain required. This slice does not complete the goal.

## Execution record

- RED: pure operation module and member copy/delete controls were absent.
- GREEN: pure copy/move/subtree/document/reference tests and member CAS/confirmation tests. A failed draft-name boundary test added the workspace-name snapshot so renaming alone also dismisses confirmation.
- Independent source review found one Important retarget race before the passive selection effect. A layout-effect regression clicked Confirm after selection changed but before passive effects; it first invoked onChange for the wrong request. Confirmations now store/compare original collection/request IDs synchronously and execute against those IDs. Reviewer found no second concrete issue and did not rerun tests.
- Full frontend passes 548 cases. Server collaboration 13, collection resource 2 and public Mock 15 pass; member deletion revokes parent/child public links, retains document source at root and rejects viewer/stale writes without giving share-management authority.
- Production frontend verifies 45 chunks; strict server Clippy and package formatting/diff checks pass. Server coverage also explicitly rejects a dangling scenario root before the fixture removes that reference. Embedded server rebuild passes.
- Runtime: English/Chinese 390 px member pages show the named scenario dependency with disabled Confirm, copy then move to an unrelated collection with exact authored-content and new-ID checks, Cancel, and a confirmed request deletion that remains local until Save. After explicit fixture reference removal, Chinese subtree confirmation names 2 collections/2 requests/1 document; Save retains the unrelated destination, reparents Guide to root without changing text, revokes the original public Mock (404) and records three member actor saves with no source content. No horizontal overflow or browser errors.
- TypeScript required an explicit collection guard in the open handler; the successful production build includes it. Locale edits preserve existing duplicate-key ordering rather than rewriting unrelated translations.
- No plan deviations or deferred independent-review findings. Reference management and the full goal remain incomplete.
