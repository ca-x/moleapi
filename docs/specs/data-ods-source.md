# ODS Data source

Authority: `data-004` in `docs/FEATURE-MATRIX.md`, `docs/specs/data-client.md`, and the existing XLSX file worker. This adds OpenDocument spreadsheets to the same explicit local upload and checked remote file flow. It does not evaluate spreadsheet formulas or create a live dataset link.

## User behavior

- `file_format=ods` is an additive Data source value. Selecting a `.ods` file chooses it automatically; the format can also be chosen for a remote file. Existing saved requests and default `csv` behavior remain compatible.
- `sheet_name` selects an exact worksheet. Empty selects the workbook's first worksheet in document order. An absent sheet fails visibly. `csv_header=true` means physical worksheet row 1 contains column names, including when earlier cells are blank; disabled header mode uses `column_1`, `column_2`, and so on.
- The editor shows the same sheet and header controls as XLSX and states that cached formula values are read without calculating formulas. SQL queries, typed results, privacy, export, and cancellation use the existing Data path.

## Parsing and limits

- Use Calamine 0.36.1 `Ods` inside the existing isolated file worker. Only an explicitly selected 5 MiB or smaller upload, or a checked remote download, enters the parser. Keep the existing 50,000 row, 256 column, 64 MiB decoded table, 512 MiB worker memory, deadline, and cancellation budgets.
- Preflight the ODS ZIP with the same 2,048 entry and 64 MiB declared expansion caps as XLSX. Reject duplicate or oversized entry names, invalid archive/mimetype, error cells, nonfinite numeric cells, blank/case-colliding headings, missing worksheets, and a used grid above one million cells. Calamine parses ODS sheets eagerly, so the worker memory limit is the final boundary for adversarial repeated rows or columns across unselected sheets.
- Preserve `null`, Boolean, and safe integral `Int64` cells. Calamine reads ODS numeric attributes as `f64`; reject integral numeric values outside the exact IEEE754 integer range instead of displaying a rounded 64-bit value. Treat floating and mixed columns as text so no extra JavaScript rounding occurs; users can explicitly cast in SQL. Read cached formula values only. Do not claim macro execution or legacy XLS/XLSB support.

## Verification

- A reproducible multi-sheet ODS fixture covers first/exact sheet, physical-first-row headers, typed/null/decimal/cached-formula values, unsafe large numeric rejection, missing sheet, duplicate/blank header, invalid ZIP, and oversized declared expansion. Execute actual SQL through DataFusion, not only parser inspection.
- Check local upload source serialization/default-private backup and a checked remote ODS download through the hosted owner-scoped API. Verify saved/reloaded 390px Chinese and English UI. Run focused Rust/frontend suites, format/Clippy, production build, branch Application CI and current platform gates.
