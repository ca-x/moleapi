# Runner Response History Implementation Plan

**Goal:** Upload bounded, already screened runner response details and persist new hosted history IDs atomically with the run report.

**Spec:** `docs/specs/runner-response-history.md`.

1. Extract the existing history privacy projection into a reusable pure function, preserving current local storage behavior. Add a transaction-scoped insertion helper for hosted completion.
2. Read redacted report-step history from the isolated local router before its temporary database is removed. Attach bounded position/entry uploads and an omission count to the completion report without printing response bodies.
3. Validate upload identity, size and position against the selected workspace and report results. Reproject and write hosted history rows in the same transaction as the report, remap links, and reject invalid uploads without partial writes.
4. Write server and real CLI process fixtures, update usage/coverage/status, then run the feature and broader verification after the remaining planned functionality and test code are complete.

The user's test-last instruction overrides the usual per-step red/green execution sequence for this slice.
