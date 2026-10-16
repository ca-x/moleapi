# Save a Data query result as a workspace dataset

Authority: remaining-work milestone 3, `data-client.md`, `SAVED-DATASETS-COVERAGE.md`, and the existing `SavedDataset`/`DatasetSource` validator. This is an explicit snapshot of a completed Data result for later runner iterations. It does not create a live database link or refresh rows automatically.

## User flow

The Data result view offers **Save as dataset** only after a successful, complete query with returned rows. The user reviews the row/column count and a short type-conversion note, enters a dataset name, and confirms. The dataset is saved through the existing workspace CAS save path and appears in the testing page's saved-dataset selector. A failed preview, version conflict, account/workspace switch or oversized result keeps the current query result visible and reports why nothing was saved. The saved SQL draft and the live Data session are not changed.

## Snapshot format and limits

- Use the existing `SavedDataset { id, name, description, secret, source }` and `DatasetSource { format:"json", source }`. No new database table or remote source mode. The new dataset defaults to `secret=true`; this save flow does not offer a public switch. The existing dataset editor can change privacy later. Do not store connection URL, credentials, SQL or query ID in dataset metadata.
- Build a JSON array of objects in displayed column order. `null`, text and boolean cells become JSON null/string/bool. Integer and decimal cells become **strings** so 64-bit integers and decimal scale cannot silently round in JavaScript runner data. Binary and truncated cells are rejected with an actionable message; users may cast/encode them in SQL and rerun. Duplicate or empty column names are rejected rather than silently overwriting object keys.
- Require a completed result that is not truncated, 1–100 rows, 1–100 columns, no binary/truncated cells, and a serialized JSON source at most 1 MiB. The existing `DatasetSource::parse` and `validate_saved_datasets` remain authoritative for per-source and workspace caps (20 datasets, 4 MiB total source). Never silently take only the first 100 rows or a visible table page; if the result is too large, ask for a narrower SQL query.
- The conversion is deterministic and bounded before the workspace draft is updated. The user can inspect/edit the resulting JSON using the existing saved-dataset editor after saving. The operation stores the values returned live by this query; file-backed rows are not recovered from the privacy-withheld history record.

## Ownership and persistence

Use the existing authenticated workspace update and expected revision. Run the existing server preview/validation before saving. Recheck account, workspace and request identity after each await, and do not apply a late result to a newly selected workspace. A successful save leaves the live Data result and saved request SQL untouched. If the workspace changed while preview or save was in flight, show a conflict and retain the name for retry.

## Evidence

Test exact large integer/decimal string conversion, null/bool/text, duplicate names, binary/truncated cells, row/column/byte caps and no silent row loss. Test creation under an owned workspace, CAS conflict, private default, restored data, runner selection by saved ID and owner isolation using existing APIs. Inspect 390px Chinese/English against a real local/remote CSV Data query and a saved/reloaded dataset. Run frontend/Rust suites, strict Clippy, production/embedded build, branch CI and the current platform gates. Extra Spreadsheet/cloud connectors, live query-backed datasets and EDAS are separate work.
