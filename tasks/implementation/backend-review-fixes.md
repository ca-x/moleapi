# Scoped backend fixes re-review

Reviewed the current source changes addressing the four findings in `backend-review.md`, the new revision tombstones, and the web bundle embedding/rebuild changes. Read-only implementation review; this report is the only file created. No subagents, source edits, commits, or target-code executions. The root's reported 40 passing tests, Clippy result, and actual served-index check were not independently rerun.

## Remaining finding

### P3 — Form percent-encoding growth is omitted from the aggregate expansion budget

`crates/core/src/interpolation.rs:98–112` charges decoded interpolated keys and values to the shared 20 MiB budget, then creates and appends the encoded pair without charging its encoding growth or the `=`/`&` separators. This does not match the new 20 MiB aggregate guarantee in `docs/API-CONTRACT.md:68`.

For example, four fields expanding to 4.5 MiB each consume 18 MiB, and a form value expanding to 1 MiB of ampersands consumes approximately another 1 MiB from the budget. The form value becomes 3 MiB after `%26` encoding, so the resolved strings total approximately 21 MiB while resolution still succeeds. All individual strings remain below 5 MiB. A template-based fixture can express this with small request inputs and valid workspace environment data.

Charge final encoded form bytes, including separators, to the remaining aggregate allowance before appending. Avoid double charging decoded temporary values when replacing their accounting with encoded bytes. Add a regression combining substantial other fields with percent-expanding form data. The existing body cap still bounds the final form to 5 MiB, so the original unbounded-allocation defect is fixed; this is a smaller accounting/contract discrepancy.

## Original findings and new changes

| Area | Source-review result |
| --- | --- |
| Unbounded interpolation | Fixed for the original allocation path: every interpolation append checks the 5 MiB per-string limit and available aggregate budget before allocation; recursive strings share a 20 MiB allowance. The encoded-form aggregate discrepancy above remains. |
| Remote recreation / false synchronization | Fixed: bases include a SHA-256 fingerprint of name/data, and remote change detection compares it as well as the revision. Bases from older versions deserialize with an absent fingerprint and are conservatively treated as changed unless content already matches. |
| Read/delete/recreate/write race | Fixed by durable monotonic revisions. Delete conditionally changes the live row to a tombstone and increments its revision; recreation conditionally changes that same tombstone back to a workspace at a higher revision. A stale write cannot match the recreated row's revision. Creation contention is checked by affected rows, and fresh creation uses the primary-key constraint. |
| Tombstone data handling | Delete replaces payload with `{}` and deletes other documents referencing the workspace, including versions, history, and sync bases, within the transaction. Reads/lists filter `kind = workspace`. Tombstones do not retain the workspace payload or snapshots through the API. |
| Tombstone schema portability | `tombstone` is nine ASCII characters and fits existing `kind VARCHAR(16)`. No schema alteration is required. The new operations use SeaORM conditional updates and transactions rather than dialect-specific SQL. The existing MySQL payload/table configuration is unchanged. |
| URI segment encoding | Fixed: UTF-8 percent encoding encodes spaces as `%20`, literal plus as `%2B`, and preserves RFC3986 unreserved characters. New workspace creation rejects `.` and `..`. The reviewed encoder also escapes percent, question mark, slash, and hash correctly. |
| Saved mock errors | Fixed: only responses successfully constructed by the mock handler receive the internal `SavedExample` extension. JSON error middleware exempts that marker, preserving saved status/body/headers without exempting authorization, extraction, routing, or missing-example errors. |
| Web bundle rebuilding | Build script watches the dist directory and each copied file, sorts traversal, hashes length-delimited relative names/content, clears stale output, and emits the digest as a compiler environment value. `env!` in the library makes the value a compilation dependency. |
| Embedded UI at runtime | `rust-embed` enables `debug-embed`, so debug builds also contain their UI bytes. Release builds still fail when production index assets are absent. Native builds without the web feature do not reference the digest or assets. No additional defect established in these scoped changes. |

## Regression coverage and limits

The added tests cover oversized single-string substitution, repeated synchronization of a space-containing ID, dot-ID rejection, divergent synchronization following remote recreation, preserving a saved 404 body/header, and stale updates after recreation. Their source matches the intended original regressions.

The new recreation test at `crates/server/tests/database.rs:247` directly constructs a native SQLite router outside the environment-driven multi-engine loop. PostgreSQL/MySQL CI therefore does not currently exercise tombstone recreation and the stale-write rejection scenario, even when its service URLs are supplied. Add that scenario to the shared engine test, ideally checking that deleted payload/history/versions are inaccessible and that conflicting recreations cannot both succeed. This is a verification gap, not an established dialect defect.

Real PostgreSQL/MySQL runtime evidence remains necessary. This scoped review does not reopen unrelated backend behavior or the future full feature matrix.
