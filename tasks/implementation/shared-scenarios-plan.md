# Shared scenario editor and saved execution

Authority: `docs/specs/workspace-collaboration.md`, canonical member scenario/resource editing and actor-attributed runs.

1. Extract controlled scenario name/description/request-step/order/condition/repeat/branch/parallel fields from the owned SavedScenarios editor. Keep its persistence and NotificationPicker in the owned wrapper; shared fields make no owner-only calls and preserve existing notification IDs.
2. Add an editor/admin shared scenario panel with collection-subtree request choices, create/edit/apply/cancel and named delete confirmation. Apply/delete changes only the canonical member draft; existing explicit shared Save performs validation/CAS/audit. Reset unapplied edits/confirmations on scope/root/saved/conflict changes, compare saved scenario identity/content before Apply, and preserve unrelated resources.
3. Run only a saved scenario for the current collection through existing member run route, preserving creator Stop, report/history and actor fences. Disable runs for unapplied edits/dirty data; disable panel editing/root changes while running. Viewer controls remain absent.
4. Tests first fail for missing shared scenario controls. Cover CAS references, branch target cleanup through reused fields, subtree filtering, remove/update references then resource deletion, notification-source preservation and no owner notification calls, cancel/delete/stale edits, saved scenario run body and Stop. Existing owned scenario/parallel tests must stay green.
5. Add real server scenario CAS/run/report actor coverage using deterministic HTTP fixtures, including viewer/stale denial. Run full frontend and affected backend suites, production build then strict server Clippy then embedded rebuild, package formatting/diff checks, independent review and real 390 px bilingual create/edit/save/run/reference cleanup before commit/push PR41.

Remaining: data/specification management, other protocols/resources, native hosted sync and current-head release gates remain required. This slice does not finish the overall goal.

## Execution record

- RED: shared scenario tab and saved-run controls absent.
- Extracted owned scenario fields, retaining owned NotificationPicker/save behavior; shared editor preserves notification IDs without mounting owner resource controls. Apply/Delete stage the shared draft, then existing shared Save validates/CAS/audits.
- GREEN: full frontend 561 cases including owned/parallel regressions, shared CAS, notification preservation, subtree choices, Cancel/Delete, six unapplied-form boundaries, saved run/Stop, viewer absence and reference-edit followed by request deletion in one draft.
- Server collaboration 13, shared scenario 1, scenario control 5, parallel 7 and saved scenarios 3 pass. The new real HTTP test executes the second request twice, stops on a false condition before the first request, preserves canonical data, returns an actor-attributed saved report and rejects viewer/stale/root-mismatch operations.
- Independent source review found no concrete issue; reviewer did not rerun tests. Production frontend verifies 45 chunks; strict server Clippy, package formatting/diff and embedded server rebuild pass.
- Runtime: actual English/Chinese 390 px member UI creates/applies/saves a repeat+conditional scenario. Two fast requests run and the false branch stops the remaining requests. Chinese reference editing followed by original request deletion persists in one valid CAS, without owner reference repair. Running the resulting slow flow admits a real 15-second request, then creator Stop returns cancelled. Two saved scenario reports and actual member run/Stop audit are verified. Both widths stay 390 px and browser errors are empty; captures are linked in collaboration coverage.
- Fixture wire assertions normalize the URI path because the engine sends an empty query marker (`/second?`); both observed paths are still `/second`, and no slow request was sent in the first run.
- Follow-up for the overall goal: audit the existing shared run UI across tab changes and local draft edits. `invalidateDraftResults` currently resets running state; the active creator job must remain stoppable until actual completion even when local edits invalidate its visible result. This pre-existing cross-tab lifecycle path is outside the verified single-panel scenario run/Stop flow.
- No deviations or deferred independent-review findings for this plan. The full goal remains incomplete.
