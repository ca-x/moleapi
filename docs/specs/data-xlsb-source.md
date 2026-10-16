# XLSB Data source

Authority: `data-004` in `docs/FEATURE-MATRIX.md`, `docs/specs/data-client.md`, and the existing XLSX/ODS/XLS file worker. This adds explicit binary Excel `.xlsb` input to local uploads and checked remote file queries.

## User behavior

- `file_format=xlsb` is additive. Selecting a `.xlsb` file infers it, and a remote source may choose it explicitly. Existing saved Data requests retain their current format.
- An exact `sheet_name` selects one worksheet; empty selects the first worksheet in document order, skipping chart/dialog/macro sheets. Header mode uses physical worksheet row 1. SQL, typed results, scripts, history, privacy and cancellation use the existing Data path.
- The editor explains that formula results come from the workbook cache and are not recalculated. Macros, external links and workbook instructions are never executed.

## Parsing and limits

- Use Calamine 0.36.1 `Xlsb` in the existing isolated file worker. Preflight the XLSB ZIP with the 2,048 entry and 64 MiB declared expansion caps used for XLSX/ODS. Retain the 5 MiB source, 50,000 row, 256 column, one-million used-grid, 64 MiB Arrow table, 512 MiB worker memory, deadline and cancellation limits.
- Reject malformed archives, missing/empty sheets, error/nonfinite cells, duplicate/blank headings and unsafe integral IEEE754 numeric values outside the exact 53-bit range. Preserve Boolean, null, text and safe integers. Float and mixed columns remain text for explicit SQL `CAST`.
- Native default backup withholds embedded workbook bytes while retaining an importable source reference; explicit full backup restores exact bytes. External exports that cannot carry the source refuse it.

## Evidence

- Use fixed upstream Calamine 0.36.1 test workbooks with exact commit/SHA256 provenance. Query actual first and named sheets through DataFusion, including header-disabled numeric rows, text and SQL filtering. Verify missing sheet, malformed ZIP, expansion and type limits.
- Verify a checked remote XLSB download through the owned hosted API, default/full native backup and a saved/reloaded 390px bilingual local upload/query. Run focused Rust/frontend suites, strict Clippy, rustfmt, production build and branch CI. Native platform packages remain a separate release gate.
