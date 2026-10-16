# XLSB Data source implementation plan

Parent: `remaining-work-plan.md` milestone 3 and `docs/specs/data-xlsb-source.md`.

1. Pin an upstream XLSB fixture with license, commit and SHA256. Add failing Data SQL and core validation tests for first/named sheet, types and archive limits.
2. Add `DataFileFormat::Xlsb`, route it through existing Calamine/Arrow conversion and apply the exact numeric guard. Preserve physical first row, worker budgets, source encoding and cancellation.
3. Add XLSB file-picker inference, format/sheet/header controls and bilingual cached-formula copy. Test a checked remote hosted query and native default/full source backup.
4. Run focused and full relevant Rust/frontend gates, strict Clippy, formatting, production/embedded build and a saved/reloaded 390px browser query. Obtain independent review, push a draft PR and inspect branch CI. Keep platform validation scoped to its exact commit.

Checkpoint: SHA-pinned upstream fixtures, local and hosted checked SQL, private/full native backup and saved/reloaded bilingual 390px browser queries are implemented. Data21, interchange59, hosted Data24 and frontend335 tests, strict scoped all-target Clippy, rustfmt, production frontend and embedded server builds passed locally. Independent review found that Calamine's sheet-name list includes chart/dialog sheets; default and named selection now filter to worksheets, with a chart-first metadata regression. Re-review found no further high-confidence issue. Branch CI remains open.
