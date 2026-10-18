# Bounded Mock fault rate

Authority: `mock-013` in `docs/FEATURE-MATRIX.md`, the existing saved-example Mock routes, call logs and delay contract, and Postman's documented Chaos Mode condition. This is a single failure-injection profile; it does not implement the Postman Simulator, dependency composition or rate limiting.

## Objective and behavior

- A saved response example has `fault_rate_percent` (integer 0–100, default 0) and `fault_status` (HTTP 500–599, default 503). Zero preserves existing behavior and omits both fields from old serialized examples. A nonzero rate draws once after an example has been selected; values 1–99 randomly inject that percentage of requests, while 100 always injects a fault. The configured 5xx status is returned with a fixed, nonprivate text body and no authored response headers. A fault does not evaluate the example's schema or script.
- Direct preview, routed Mock, native listener and authorized public Mock use the same selected-example path. Existing public share admission, owner/revision and revocation gates still apply. A configured delay applies to the injected response as well as the normal response and shares the existing 32-slot bound, cancellation behavior and post-delay revision check. Unmatched requests and requests rejected before example selection never draw. A selected call can be sampled before later delay-capacity or response-construction failure; such failures return their existing errors and are never logged as simulated faults.
- The Mock call log records the actual response status and `simulated_failure` outcome without logging request data or the random draw. A zero-rate response retains its existing body/status/script/log behavior. Full and default MoleAPI backups preserve these nonsecret settings; external formats reject nondefault settings until they have an exact mapping.
- The bilingual example editor labels the percentage and 5xx status clearly, constrains them before save and shows saved values after reload. Changing either field invalidates the current preview. At 390px both controls remain reachable with no document horizontal overflow.

## Validation and boundaries

- Core save validation rejects rates above 100 and statuses outside 500–599. Old examples deserialize without migration. The fault status remains validated even when the rate is zero, so a saved draft cannot turn invalid when enabled later. Server checks persisted data defensively before using it.
- The draw uses the server's existing `rand` dependency. It is independent per selected call and does not store a seed or request data. No deterministic distribution, cluster-wide rate guarantee, dependency fault orchestration, bandwidth/connection errors or per-minute quota is claimed.
- Existing fixed 4xx/5xx examples, scripts and schema generation retain their current meaning. The fault configuration is opt-in; no failure is injected for old data.

## Project structure, style and commands

- `crates/core/src/models.rs` and `validation.rs`: serde contract and bounded validation.
- `crates/server/src/mock.rs`: one shared selected-example decision; `crates/server/tests/mock.rs` and public/native tests verify real routes and logs.
- `crates/formats/src/lib.rs` and import constructors: backup/external export behavior.
- `web/src/features/requests/ExamplesEditor.tsx`, shared types and bilingual catalogs: editor controls.
- `docs/MOCK-FAULT-RATE-COVERAGE.md`: exact evidence and open limits.

Follow the existing `Example` field/default pattern, Rust rustfmt/Clippy and React/Radix `Field` controls. Build frontend before embedded Rust binaries. Verify with `cargo test -p moleapi-core -p moleapi-formats -p moleapi-server`, `cargo clippy -p moleapi-core -p moleapi-formats -p moleapi-server --all-targets -- -D warnings`, `cargo fmt --check`, `npm --prefix web test -- --run`, `npm --prefix web run build`, then `cargo build -p moleapi-server`. Exercise real owner/public/native Mock calls and a 390px bilingual save/reload/preview before claiming the slice locally verified. Fixed-head three-database, Docker and native package checks remain separate gates.
