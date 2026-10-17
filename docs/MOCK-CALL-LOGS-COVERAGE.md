# Owned Mock call log coverage

Scope: `mock-008`, `docs/specs/mock-call-logs.md`, branch `feat/mock-call-logs` stacked on the request matching branch. These logs are metadata records, not request/response traffic capture.

The direct saved-example and routed Mock endpoints durably record a selected response, unmatched call or generic failure after workspace ownership is established. Records contain IDs, canonical method, status, outcome, UTC timestamp and elapsed milliseconds. They never contain incoming path/query/headers/body, bearer tokens, script error text, dataset rows or response bytes. The owner-only list/clear API is under `/api/workspaces/{id}/mock-calls`; the bilingual example editor shows recent calls for the current request plus unmatched calls and requires a second click to clear all workspace calls.

An owner/workspace row lock and atomic sequence document serialize append, prune and clear across instances. At most 200 records remain per workspace; workspace deletion cascades both records and sequence state. A write failure withholds the selected Mock response. Calls denied before workspace ownership is established and calls invalidated by workspace deletion/revision change may have no log.

Local evidence: five focused hosted/native server tests pass for owner isolation, 240 calls through two routers retaining the latest 200 sequences, deletion cascade, clear, generic error/no-match outcomes, private value exclusion, and a forced SQLite insert failure withholding the response. The configured database integration test now exercises a direct Mock and concurrent calls from separate routers under the same owner on SQLite; PostgreSQL/MySQL require branch CI. The server suite passed 317 tests with eight existing ignores when the one local Java catalog test requiring an unprepared OpenAPI Generator JAR was excluded. Full frontend suite: 360 passed; strict scoped Clippy, production frontend and embedded server builds passed.

Actual 390px hosted browser: previewed a saved Mock, refreshed the activity list, reopened the workbench to confirm persistence, then cleared logs with the two-click control. English/Chinese states rendered at document width 390px; final controls measure 44px and 52px high. Current-head three-database CI, native packages and independent review remain open.

Remaining Mock goals include schema-driven data, public permissions, simulated delays/failures and richer traffic inspection with explicit privacy controls. The complete 395-row product matrix is not claimed complete.
