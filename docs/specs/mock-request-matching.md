# Owner-scoped Mock request matching

Authority: `mock-004` and `mock-005` in `docs/FEATURE-MATRIX.md`, the existing saved-example route, and `docs/specs/mock-scripts.md`. This adds dispatch by an actual incoming HTTP request while retaining the stable explicit example URL.

## Contract

- `/api/mocks/{workspace_id}/{*path}` requires the workspace owner's bearer token. It accepts ordinary HTTP methods, a bounded path/query/header/body snapshot, scans at most 10,000 saved requests and 10,000 examples per dispatch, and never performs a network request. The existing `/api/mock/{workspace_id}/{request_id}/{example_id}` route keeps its current direct-example behavior.
- Candidate saved requests must have HTTP protocol, a literal absolute HTTP(S) URL, matching method and matching path. A path segment written as `{name}` in the saved URL matches one nonempty incoming segment. Query pairs already present in the saved URL or enabled request query rows must match; query pairs, headers and body are also optional per-example `mock_match` conditions. All enabled conditions must match. Query/header pairs use multiset matching so duplicates matter. Header names compare without case; values and body bytes compare exactly. Incoming owner credentials, cookies, hop-by-hop fields and sensitive query keys are excluded from both matching and the script snapshot.
- Matching examples outrank fallbacks with no `mock_match`. Among matches, higher `priority` wins, then more literal path segments, then more conditions, then authored collection/request/example order. Missing match returns 404. A malformed or oversized incoming request fails closed. Never execute an unselected example's script or dataset query.
- The selected example uses the existing static/dynamic response path and safe response-header filter. A v2 Mock script sees the actual incoming method, path, safe query/headers and bounded UTF-8 body. Owner/revision changes, worker failures and privacy failures never serve a fallback body. Explicit selected-example preview remains available.
- `mock_match` is additive and optional; old workspaces and backups retain their explicit example URL behavior. Validate at save time: at most 20 query and 20 header conditions, valid literal header names/values, at most 1 MiB body criterion, priority 0–100, and at least one condition when a match rule exists. Exclude credentials and hop-by-hop header keys from authored conditions. The bilingual editor exposes method/path and per-example conditions/priority without hiding unsupported saved URL templates.

## Evidence

- Core old JSON compatibility and invalid-rule rejection; full/default backup privacy for match criteria.
- Real owner route: method/path, path segment placeholders, repeated query pairs, case-insensitive headers, exact body, specificity/priority/tie order, static and dynamic selected examples, owner denial, credential exclusion, size limits and revision race.
- Bilingual 390px save/reload/dispatch/preview; focused Rust/frontend suites, strict Clippy/format/build, three-database CI and current native package checks before matrix support claims.

## Remaining Mock scope

Schema-driven generation, call logs, public sharing, simulated delay/failures and local standalone Mock ingress remain separate rows. Completing this matching slice does not close the full Mock module.
