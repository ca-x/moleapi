# Runner response history upload

## Objective

Make response details from a self-hosted runner available through the same owned report-step response endpoint as local runs. The agent's temporary SQLite database is deleted after execution, so its local history identifiers cannot be retained by the hosted report.

## Contract

- The native agent reads each saved response from its isolated local `/api/workspaces/{workspace}/reports/{report}/steps/{position}/response` endpoint after a run. That endpoint returns the already privacy-screened `HistoryEntry`; the agent never uploads live response objects directly. The agent attaches bounded `history_uploads` containing `{position,entry}` to its completion report. A report without a saved local report or linked response remains valid but has no response-detail links.
- At most 8 MiB of serialized upload entries may be attached to one completion, with at most 1000 positions and one entry per position. The agent omits entries exceeding the budget and reports the omission count in CLI metadata. It retains no raw response in stdout. The server rejects malformed, duplicate, wrong-workspace or wrong-request entries atomically.
- The hosted completion transaction validates the runner's scoped bearer and lease as before. It ignores agent-supplied metadata except the entry's response and its match to the result's local history ID. The server reapplies its own workspace/environment privacy projector, generates new hosted history IDs and writes history rows and the report in one transaction. On any failure, no history row or report is committed. Repeated completion returns the previous task without inserting duplicates.
- Unuploaded step identifiers are removed before report projection, so no report link points into deleted runner storage. Existing owner/workspace checks on report-step response and clear-history behavior remain unchanged. Binary and uploaded-file privacy rules remain those of the shared history projector. The server still trusts the runner to execute selected requests and report results; the selected workspace's request secrets are visible to that runner.

## Acceptance

Write local CLI/HTTP and dual-router tests for linked redacted details, unknown/duplicate/oversized/mismatched entries, omission and idempotence, no partial writes, revoked leases, owner isolation and clear-history behavior. Write UI coverage only if the existing report detail requires a change. Defer test execution until after functionality and test authoring, as instructed by the user.
