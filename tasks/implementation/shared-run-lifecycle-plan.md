# Shared active-run lifecycle

Authority: `docs/specs/workspace-collaboration.md`, creator-bound Stop and scoped result invalidation. Follows the concrete cross-tab/draft issue recorded in `shared-scenarios-plan.md`.

1. Reproduce disappearing Stop after local edits, collection selection/deletion, Save and remote canonical changes; verify a second run remains blocked until the original completes. Keep current account/credential/role/workspace exit fences.
2. Preserve the active job independently from visible result generation. Draft/source changes invalidate old output, but keep the original job ID/label until its own promise settles. Final cleanup compares the exact active job identity, so late prior completion cannot clear a newer run. Refresh saved history/reports for completed same-scope runs even if their inline result was discarded.
3. Render one workspace-level run/Stop region across resource/docs/variables/scenario/history/report/Mock tabs, including an empty/deleted collection. Remove tab-local duplicate Stop controls. Stop acknowledgment does not mark execution complete; completion/error or scope loss cleans it up.
4. Regression tests cover edit/Save/remote/selection/deletion, tab navigation, no second dispatch, stale output withholding, Stop job ID, retry after completion/failure, and account/role/credential/workspace plus late-run/Stop boundaries. Preserve existing scenario and collection tests.
5. Run full frontend, relevant server collaboration/scenario suites, production build then strict server Clippy then embedded rebuild sequentially, formatting/diff checks, independent review and real English/Chinese 390 px edit/save/tab/Stop acceptance. Record evidence and push PR41.

Full goal remains active: data/specification management, other resource/protocol parity, native sync/installed acceptance and current-head release gates remain required.

## Execution record

- RED: six tests first failed because Stop disappeared after draft edit, Save, selection, deletion, remote revision or page navigation. Root cause: output invalidation reset `running`, and the generation-gated finally skipped cleanup after an edit.
- Fixed: preserve the active job separately from visible result generation, retain source label/job ID, release only the exact finishing job, block a second run by active ref, refresh saved results on same-scope success and render one shared Stop region. Old account/credential/role/owner/workspace callbacks remain fenced.
- GREEN: focused shared-view 117 and full frontend 576 pass; new tests also cover later success/failure cleanup, report refresh after invalidation, five scope boundaries, old completion versus a new run and a late Stop acknowledgment versus a newer stopping job.
- Server collaboration 13, collection resources 2 and shared scenarios 1 pass. Independent source review found no concrete regression; reviewer did not rerun tests.
- Production frontend verifies 45 chunks; strict server Clippy, package formatting/diff and embedded server rebuild pass.
- Runtime: actual English/Chinese 390 px member flows admit a real slow HTTP request, edit/Save then switch to Docs, and delete/Save the original collection then switch to Variables. One Stop region still names the original collection in both cases and cancels the matching original job. Two saved cancelled reports, member run/Stop audit IDs, canonical edits and document relocation are verified. Neither view overflows and browser errors are empty; captures are linked in collaboration coverage.
- No plan deviations or deferred review findings. Full goal remains incomplete.
