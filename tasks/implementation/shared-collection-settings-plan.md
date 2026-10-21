# Shared collection and folder settings

Authority: `docs/specs/workspace-collaboration.md`, member resource editor parity and canonical workspace CAS.

1. Extract controlled collection settings fields from the owned dialog: name/parent, inherited auth, shared variables/enabled flag and pre/post scripts. Keep owned variable reconciliation and token-management behavior in its wrapper.
2. Add a shared dialog for root/child creation and editing. Reuse tree cycle/depth checks and retain current requests when metadata is applied. Disable owned token management, strip member local variable overrides and preserve unrelated workspace edits. Apply changes only to the local member draft; explicit shared Save uses existing audited CAS.
3. Expose root creation even with zero collections, child creation and selected collection settings. Viewer has no mutation controls. Close stale dialogs after account/credential/role/workspace/saved/conflict revision changes; an unapplied edit never survives to another scope.
4. Tests first fail for missing empty-workspace creation/settings controls. Cover create/cancel, settings CAS, viewer denial, inherited auth/scripts/variables and preserving requests; cycle/depth rules and stale dialogs. Existing owned dialog tests must remain green after extraction.
5. Run focused/full frontend and relevant server collaboration tests, production build then strict server Clippy then embedded build sequentially, package formatting/diff checks, independent source review and real bilingual 390 px member create/edit/save/inheritance acceptance. Record evidence and push PR41.

Remaining: destructive collection/request resource controls, protocol/resource editor parity, native hosted sync and full release gates remain required subsequent work.

## Execution record

- RED: shared creation/settings tests failed for absent controls.
- Extracted fields preserve owned controls and owned variable reconciliation. Shared wrapper merges current requests, rejects concurrent metadata, strips local variable overrides and never mounts owned OAuth token managers.
- Focused tests cover empty workspace/root/child create, Cancel, scripts CAS, viewer denial, current request preservation, variable flag/override handling, OAuth1/2 configuration without an owned provider, parent exclusions, metadata conflict and five stale-dialog boundaries. Full frontend passes 531 tests.
- Real server test passes shared CAS creation of a parent/child tree and actual inherited variables/auth/parent+child scripts on HTTP wire; viewer write, stale revision and cycle are denied. Existing collaboration 13 also pass.
- Independent source review found no concrete stale apply, request preservation, OAuth control or owned extraction regression; reviewer did not rerun tests.
- Production frontend verifies 45 chunks; strict server Clippy, package formatting, diff check and embedded server rebuild pass. TypeScript caught three unused mock path arguments, now named `_path`; the successful build includes that correction.
- Runtime: actual English/Chinese 390 px member UI starts from an empty synthetic workspace, creates parent/child folders and a saved request, and receives substituted parent variable plus inherited bearer auth and both scripts on the HTTP wire. Renaming/moving the child to root and back preserves its request. Canonical owner reads and four actor-labelled member saves are verified without secret audit content. A remote owner revision closes the unapplied dialog and discards its name. Both widths stay 390 px, dialog width is 366 px, browser errors are empty. Captures are linked in the collaboration coverage document.
- No plan deviations or deferred independent review findings. Destructive resource controls and the full goal remain incomplete.
