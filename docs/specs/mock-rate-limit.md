# Shared Mock request rate limit

Authority: `mock-013` in `docs/FEATURE-MATRIX.md`, the existing saved-example Mock selection/log/delay/public routes, and Postman's documented “maximum requests allowed per minute” simulation condition. This adds one condition to saved examples; it does not implement the full Postman Simulator or global dependency orchestration.

## Objective and wire contract

- A saved example gains `rate_limit_per_minute` (integer 0–1,000, default 0). Zero preserves old behavior. For a selected example with a positive limit, at most that many calls are admitted in any sliding 60-second window across all server instances sharing the same database. Only admitted calls consume quota. The next call returns HTTP 429 immediately, with `Retry-After` rounded up to at least one second, a fixed nonprivate text body and no authored headers/body/schema/script. Its private call log outcome is `rate_limited`, separate from execution failure and injected 5xx fault.
- Admission occurs after saved-example matching and public-share authorization, before fault sampling, schema generation, scripts or configured response delay. A rejected call does not draw a simulated fault or use a delay slot. Successful calls retain their existing fault/delay/body behavior. A workspace revision change, deletion or revoked public share must prevent stale output; quota admission fails closed when its database transaction fails.
- A workspace save resets its rate state in the same transaction as the revision update. Workspace deletion removes rate state with other owner-scoped documents. Native default/full backups preserve the nonsecret setting; external Postman/OpenAPI/HAR formats reject a nonzero limit until an exact mapping exists. Runtime timestamps and counts are never included in workspace backup/sync or call logs.
- The bilingual saved-example editor has a labeled integer input with 0 meaning disabled, clamps typed values to 0–1,000, invalidates preview on edit and shows the saved value after reload. A 390px viewport can reach the control without document horizontal overflow. Logs display “Rate limited”/“已限流” rather than generic failure.

## Consistency and bounds

- Use the existing owner/workspace document row as a transactional serialization point, as Mock call logs already do on SQLite, PostgreSQL and MySQL. Within that transaction, read the database server's UTC epoch time, prune timestamps older than 60 seconds, compare count to the saved limit, then append one timestamp and commit before serving. One owner/workspace/request/example counter document stores at most 1,000 timestamps. Do not use a process-local counter or application-node clock for admission.
- Counter IDs hash owner/workspace/request/example so cross-account values cannot collide. All queries filter owner, kind and workspace ref. Corrupt state fails closed. Workspace save clears counters transactionally, which also bounds orphan state after deleting or replacing examples. A public share's existing revocation/revision check remains after response construction.
- The 429 response bypasses saved delay. Accepted calls that later fail scripts or are cancelled still consume an attempt, matching admission-time request counting. Database or network errors are not counted as successful Mock responses in the log. A database clock jump may alter the rolling window; this is an operational clock property, not a cluster-wide monotonic-clock claim.

## Project structure, style and verification

- `crates/core/src/models.rs` and `validation.rs`: optional field and bounded save contract.
- `crates/server/src/mock_rate.rs`: owner-scoped transactional admission and cleanup; `mock.rs` uses it after selection, and `storage::replace_in` clears state in the workspace CAS transaction.
- `crates/server/tests`: real direct/routed/public/native 429, Retry-After, cross-instance concurrency, owner/revision/deletion and log tests against configured databases.
- `crates/formats`: native backup roundtrip and external-export gates.
- `web/src/features/requests`: bilingual editor/log labels and focused interaction tests.
- `docs/MOCK-RATE-LIMIT-COVERAGE.md`: exact supported combinations, evidence and limits.

Follow existing `Example` serde defaults, SeaORM transaction and Radix `Field` patterns. Run `cargo test -p moleapi-core -p moleapi-formats -p moleapi-server`, strict all-target Clippy for those crates, rustfmt,  full frontend tests/typecheck/build, then embedded server build. Verify 390px bilingual save/reload and actual 429/Retry-After on a hosted browser. Fixed-head SQLite/PostgreSQL/MySQL, Docker and native packages remain release gates.
