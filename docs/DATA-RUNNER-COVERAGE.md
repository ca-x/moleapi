# Dataset-driven collection runner

Hosted and offline collection execution accept ephemeral CSV/JSON data, configurable iterations and cancellation. Existing single-run payloads remain compatible. CSV uses csv1.4.0; JSON uses serde_json. Sources are supplied text, never server file paths or URLs.

POST `/api/workspaces/{workspace}/run` accepts optional `dataset:{format,source}`, `iterations` and caller-generated `job_id`. Omitted iterations uses all rows or one run. Explicit counts select the first N rows and cannot exceed the dataset. Row values override existing execution data; temporary overrides retain higher priority. Interpolation uses strings/JSON encoding. Read-only `pm.iterationData` returns original JSON types with copied getters; CSV stays strings. `pm.info` exposes zero-based iteration, iterationCount, requestId, requestName and eventName. Script variable changes chain within the run without altering the canonical workspace.

POST `/api/testing/dataset/preview` validates workspace ownership and returns typed rows/columns. The testing page imports bounded CSV/JSON/text files or edited source, previews up to10 rows, sets iteration count, stops runs and displays iteration summaries. Sources/results and pending selections are account/workspace fenced. Result collection_id routes browser local updates to the actual request collection.

POST `/api/workspaces/{workspace}/run/cancel` cancels owned work using job_id. A bounded30s early-cancellation marker handles stop-before-registration races. Logout/workspace deletion share cancellation. Stopping during a pending save prevents subsequent submission. Completed requests are not rolled back or mislabeled.

Limits:1 MiB source,100 rows/columns,64 KiB encoded cells,50k nodes/64 levels,100 iterations,1000 executions and300s deadline. All rows are checked against initial variable budgets before network access. Reports retain counts/identifiers while explicitly omitting oversized response details under an8 MiB limit; omitted responses cannot replay their variable-update details into browser local overrides. Nested dataset leaf values are captured for saved-history privacy. Previews/responses are live-only; data is never implicitly persisted.

Evidence: quoted/newline CSV and typed JSON fixtures; copied/read-only script access and iteration metadata; actual row interpolation, mutation chaining, nested history privacy and unchanged workspace data; in-flight/pre-start cancellation; report omission without lost counts; later-row prevalidation before any network request; legacy runner assertion/error behavior. Frontend cases cover preview/limits, stale owner file choices, stopping during save and collection-aware local updates. Scoped Clippy, TypeScript and production frontend build passed. No local Docker builds were used.

Persistent/reusable datasets, scenario branch/loop/parallel graphs, schedules/remote runners and durable report publication/export remain required. Native OS picker and new platform/database checks are not established by these fixtures. Overall parity and final release remain open.

Named sources can now be persisted in workspace datasets and selected by dataset_id, with the same runtime semantics. See SAVED-DATASETS-COVERAGE.md for storage, backup privacy and sync evidence; query-backed/remote sources and the other scenario/report requirements remain open.
