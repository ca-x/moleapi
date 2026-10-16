# Legacy XLS Data source

Authority: `data-004` in `docs/FEATURE-MATRIX.md`, `docs/specs/data-client.md`, and the existing XLSX/ODS file worker. This adds explicit BIFF8 `.xls` input to the same local upload and checked remote file query flow.

## User behavior

- `file_format=xls` is additive. Selecting a `.xls` file infers it; a remote source can select it explicitly. Existing saved requests keep their current format.
- An exact `sheet_name` selects one worksheet; empty selects the first in workbook order. Header mode uses physical worksheet row 1. The same SQL, result, history, privacy and cancellation path applies.
- The editor explains that formula results come from the workbook cache and are not recalculated; a formula with an empty cache stays empty. VBA/macros, external links and workbook instructions are never executed.

## Parsing and limits

- Use Calamine 0.36.1's `Xls` reader inside the existing 512 MiB isolated file worker. Input remains capped at 5 MiB, selected rows at 50,000, columns at 256, grid cells at one million and decoded Arrow data at 64 MiB. XLS is an OLE compound file, so the ZIP entry preflight for XLSX/ODS does not apply. A bounded CFB/BIFF8 record preflight checks every worksheet's declared and actual grid before Calamine eagerly materializes any worksheet. Non-worksheet BIFF8 substreams and older BIFF variants reject explicitly.
- Reject malformed workbooks, absent sheets, error/nonfinite cells and invalid headings. Preserve booleans, nulls, strings and safe integral numbers. Floating or mixed columns become text so SQL `CAST` is explicit. XLS numeric cells are IEEE754 values; an integer outside the exact 53-bit range must reject rather than silently round, including when a numeric cell is date-formatted.
- Keep source bytes in native backup; default export follows existing private file-source projection. Other external formats must not claim to preserve XLS source bytes.

## Verification

- A reproducible BIFF8 fixture with multiple worksheets proves first/exact sheet selection, header mode, typed/null/text/decimal values, an empty cached formula and actual SQL. Malformed input, missing sheet, bad headers, unsafe date-formatted numbers and an oversized unselected sheet reject before eager parsing.
- Exercise local and checked remote sources through the owned hosted API, plus a saved/reloaded 390px bilingual UI flow. Run focused Rust/frontend suites, strict Clippy, format, production build and branch CI. Final native package/database gates remain part of the broader release plan.
