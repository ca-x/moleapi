# XLS Data source implementation plan

Parent: `remaining-work-plan.md` milestone 3 and `docs/specs/data-xls-source.md`.

1. Generate a pinned, reproducible two-sheet BIFF8 fixture and write failing Data SQL and core validation tests.
2. Extend `DataFileFormat` and the existing Calamine spreadsheet conversion with `Xls`; apply the same row/column/grid, numeric and worker limits while skipping only ZIP-specific preflight. Reject unsupported workbook content explicitly.
3. Add XLS file inference, worksheet/header controls, bilingual cached-formula copy and focused UI tests. Verify owned local and checked remote hosted queries plus private/full backup.
4. Run Rust/frontend/production gates and a real 390px saved/reloaded browser query. Update Data coverage, request branch CI and keep package/database evidence scoped to its exact commit.

Checkpoint: the BIFF8 fixture, local/hosted SQL, private/full export, bilingual saved/reloaded 390px browser query, 15 Data/21 core/58 interchange/23 hosted Data/334 frontend tests, strict scoped all-target Clippy, rustfmt and frontend/embedded server builds passed locally. Independent review and branch CI remain open.
