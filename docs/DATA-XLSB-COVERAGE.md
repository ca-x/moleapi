# XLSB Data source coverage

Scope: `data-004`, `docs/specs/data-xlsb-source.md`. `file_format=xlsb` routes selected local uploads and checked remote downloads through the existing isolated Data file worker, Calamine 0.36.1, Arrow and DataFusion. It is additive to CSV/JSON/Parquet/XLSX/ODS/XLS.

| Area | Evidence |
| --- | --- |
| Parser and SQL | Two SHA-pinned upstream Calamine XLSB workbooks provide first and named sheets, header-disabled numeric rows and mixed values. Actual SQL selected `2 / b` from `issue2`; `Visible` and `datatypes` covered text, safe integers, decimal text and Boolean conversion. A chart-first metadata regression verifies default selection skips non-worksheet sheets. Missing/empty sheet, malformed ZIP and declared expansion limit reject. The first physical row becomes a header when enabled. |
| Source and privacy | `sheet_name` and `file_format=xlsb` survive native source serialization. Default native backup retains a usable sheet reference while withholding workbook bytes; explicit backup restores exact bytes. Unsupported Postman export rejects. |
| Hosted and browser | An owned hosted session downloaded XLSB from a checked DNS-pinned HTTP endpoint and queried its named sheet. The full hosted Data client suite passed 24 tests. A real 390px English/Chinese browser uploaded XLSB, selected the sheet, saved/reloaded the request, then queried `2 / b` in both languages. `documentElement.scrollWidth` equaled the 390px viewport. |
| Local gates | Data 21, interchange 59, hosted Data 24 and frontend 335 tests passed. Strict all-target Clippy on the touched Rust packages, rustfmt, patched frontend production build and embedded server build passed after the metadata review fix. The earlier 390px browser query uses a named worksheet, whose product route is covered again by the current hosted suite. |

The parser retains the existing 5 MiB source, 2,048 ZIP entry, 64 MiB declared ZIP expansion, 512 MiB isolated worker, 50,000 row, 256 column, one-million used-grid and 64 MiB Arrow table limits. Formula calculation, macro execution, arbitrary malformed XLSB variants, current platform installers and branch CI are not covered by this evidence. The exact fixture source, license and SHA256 values are in `crates/data/tests/fixtures/XLSB-PROVENANCE.md`.
