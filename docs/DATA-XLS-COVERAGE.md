# Legacy XLS Data source coverage

Scope: `data-004`, `docs/specs/data-xls-source.md`. `file_format=xls` routes selected local uploads and checked remote downloads through the existing Data file worker, Calamine 0.36.1, Arrow and DataFusion. It is additive to CSV/JSON/Parquet/XLSX/ODS.

| Area | Evidence |
| --- | --- |
| Parser and SQL | A reproducible `xlwt==1.3.0` BIFF8 fixture has first and named sheets, integer/decimal/Boolean/null/text cells and an empty cached formula. Actual SQL returned `2 / 2.30 / café`; the formula cache stayed empty instead of being evaluated. Missing sheets, duplicate/blank headers, unsafe integral numeric values, malformed input and an oversized used grid reject. |
| Source and privacy | `sheet_name` and `file_format=xls` survive native source serialization. Default native backup retains an importable sheet reference while withholding workbook bytes; explicit backup restores exact bytes. Unsupported Postman export rejects. |
| Hosted and browser | An owned hosted session downloaded XLS from a checked DNS-pinned HTTP endpoint and queried its named sheet. The full hosted Data client suite passed 23 tests. A real 390px English/Chinese browser uploaded XLS, selected the sheet, saved/reloaded the request, then queried the same exact row in both languages. `documentElement.scrollWidth` equaled the 390px viewport. |
| Local gates | Data 15, core 21, interchange 58 and frontend 334 tests passed. Strict all-target Clippy on the touched Rust packages, package rustfmt, patched frontend production build and embedded server build passed. |

The parser is bounded by the existing 5 MiB source, 512 MiB isolated worker, 50,000 row, 256 column, one-million used-grid and 64 MiB Arrow table limits. XLS is OLE, so XLSX/ODS ZIP preflight does not apply. Macro execution, formula calculation, XLSB, arbitrary legacy workbook variants, current platform installers and branch CI are not covered by this evidence.
