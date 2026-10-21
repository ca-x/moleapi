# Shared query result to dataset plan

> Use executing-plans and verification-before-completion.

**Goal:** Save all returned shared Data query rows as a private canonical dataset usable by run/Mock consumers.
**Spec:** docs/specs/workspace-collaboration.md; full feature matrix remains active.
**Architecture:** Reuse resultDatasetSource and shared member preview/CAS. Add result-scoped dialog, pass parent save callback through live session controls; reject dirty/conflicted/stale contexts and preserve subsequent local edits after a pending write.

## Constraints / review focus

- Preserve exact integer/decimal strings, booleans/null; snapshot all returned rows independent of table filter/page/sort.
- Reject failed/incomplete/truncated/empty/binary/duplicate columns, >100 rows/columns, >1 MiB source, >20 datasets, >4 MiB aggregate, invalid UTF-8 byte title length.
- Private by default; no owner routes; member preview expected_revision then CAS same canonical revision.
- No dispatch after dialog/result/scope/cache version changes while preview pending; safe same-ID retry; no dirty draft overwrite.
- Concurrent edits while PUT is pending survive and retain newly saved dataset. Role/credential/version/tab changes discard private dialog state.

## Task 1

- [x] RED shared Save button/dialog/payload and result validation tests.
- [x] Shared dialog/member callback; reuse owned conversion and messages.
- [x] Test success, retry/failure, concurrency and stale scope boundaries; affected server persistence tests.

## Task 2

- [x] Full frontend/build/server gates, independent review.
- [x] Bilingual 390px real result → dataset → shared run/Mock consumer acceptance.
- [x] Docs/commit/push/PR and CI observation.

## Evidence ledger

Task 1: shared Save RED absent button; GREEN exact full typed rows via member preview/CAS. Canonical response missing dataset RED false success; GREEN response confirmation before adoption. Full frontend622/125, server43. Name/quotas, same-ID retry, six delayed-preview boundaries, pending-write local edits and committed-but-lost response/refetch reconciliation covered.
Final review: shared_report_export_review found no concrete issue, did not run tests. Reviewer left real network response-loss recovery unjudged; an additional mocked committed-write/refetch regression passes without a second PUT.
Task 2: production45 chunks, TypeScript, strict Clippy, package format/diff, embedded build pass. Real bilingual390 query/filter/Save plus exact two-row canonical source, two HTTP consumer calls, SELECT view/Mock generated full typed rows, three content-free actor updates, Cancel/no duplicate, no overflow/errors.
Final: Ruling: real dropped TCP response after server commit was not induced — mock commit-with-rejected-response plus canonical refetch verifies conservative recovery and no duplicate; ordinary browser HTTP persistence verified end to end — cost if wrong: browser-specific reconnect feedback may require further handling.
