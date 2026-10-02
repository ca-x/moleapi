# Backend foundation implementation report

Implemented in `/home/czyt/code/rust/moleapi/.worktrees/application`, without commits or edits to root-owned application sources. This report covers the foundation backend and HTTP/IPC contract. It does not claim completion of the full research feature matrix.

## Module boundaries

- `crates/core/src`: shared models, network policy, variable interpolation, request/workspace validation, assertion evaluation, transport, URL redaction.
- `crates/server/src`: authentication, workspaces, execution, history, collection runner, saved-example mocks, formats adapter, native synchronization, router composition, standalone CLI.
- `crates/server/src/storage`: SeaORM connection configuration, SeaQuery/sea-orm-migration schema, portable document and revision operations. Account/session/settings/document entities reside in `entities.rs`.
- Integration suites separated into database, auth, execution/history, formats, mock, native/assets, runner, and sync features.

## Implemented behavior

### Core execution

HTTP/S only; supported method/body/auth kinds and timeout bounds validated. Form bodies decode fields before interpolation and encode resolved keys/values afterwards, preserving literal delimiters and handling percent-encoded template tokens. All string request fields interpolate enabled environment variables and reject unknown/malformed/nested unresolved templates with a generic error. Workspace validation checks schema version, IDs, active environment and canonical specification references. Original specification sources and operation links use backward-compatible serde defaults.

Hosted request policy rejects private/reserved IPv4 and IPv6, including mapped IPv6. Every connection/redirect resolves DNS, checks all returned addresses, and pins the validated addresses into a no-proxy client. Redirects are manual, capped, disallow HTTPS downgrade and non-HTTP schemes, preserve 307/308 methods/bodies, apply 301/302/303 method semantics, and strip authorization/cookie headers across origins. Transport framing headers are controlled by the client.

Body size is capped at 5 MiB; total request timeout is bounded to 120 s. Response consumption is streamed, capped at 5 MiB, and marks truncation. Automatic decompression is disabled to avoid decompression amplification; compressed/binary payloads are preserved, with base64 for invalid UTF-8. Status/duration/contains/JSON pointer assertions return typed results, and malformed JSON expectations/pointer escapes are rejected.

### Storage and hosted authentication

SeaORM and sea-orm-migration use SQLite/PostgreSQL/MySQL drivers with Tokio/rustls. SQL business operations use the same entities, filters, transactions and affected-row checks for all engines. SQLite uses foreign keys, WAL, a busy timeout and one pooled writer; PostgreSQL/MySQL connections use UTC; MySQL migration tables use InnoDB/utf8mb4 and LONGTEXT for workspace/history payload capacity. The only dialect branch is schema/connection configuration, not business logic.

Registration hashes passwords using Argon2 off the async executor. A transactional update to a unique singleton settings row serializes first-admin registration; the first successful registration requires the setup token, later registration respects policy, and username uniqueness is enforced by the database. Session tokens contain 256 random bits, persist only SHA-256 digests, expire after 30 days, and are deleted on logout. Hosted APIs authorize workspace/history/version/execution/runner/mock/export access by account; hosted CORS is not opened.

Workspace updates use an atomic conditional update matching owner/ID/revision. Snapshot insertion and revision changes commit together. Stale writes/deletes return 409. Workspace IDs may be reused independently across tenants. History contains no request auth/header/body, redacts sensitive query values in both entry and stored response URLs, and redacts set-cookie response headers. Endpoint-returned response bodies remain recorded, as specified.

### Native synchronization and packaging

`local(Path)` opens only the API router, never a listening socket. Its SQLite database is created/chmodded to 0600 on Unix. Paths containing query/fragment/percent characters are encoded safely. Hosted native-only paths return 404 even without a bearer token.

Native connections require HTTPS except loopback HTTP, have DNS/connect/request timeouts, pin resolved addresses, disallow redirects, and limit remote response consumption. Remote session tokens stay in native SQLite and never appear in status responses. Server/account switching deletes connection bases while retaining workspaces.

Durable per-workspace bases track local revision, remote revision, and connection identity. Missing remote workspaces can be created; equal states establish bases; local-only changes push; remote-only changes pull; divergent changes return a conflict without overwriting. Explicit pulls snapshot local state; explicit pushes still use fetched remote revision CAS. Final local workspace/base commits are atomic and reject local changes or connection changes during network waits. A local workspace comparison also fences deletion/recreation with the same revision.

The standalone binary supports bind/database path/database URL/options and MOLEAPI_* environment configuration, gracefully shuts down on SIGINT/SIGTERM, and prints setup credentials only while setup is required. Default web builds embed a copied `web/dist` via rust-embed; missing production assets fail at build time. Debug-only fallback assets provide a setup hint. The server library builds with no-default-features for desktop IPC. API paths never receive SPA fallback responses. Framework request errors are normalized to JSON.

Formats API routes call root-owned `moleapi-formats` in blocking workers to avoid blocking async request processing on larger conversions.

## Verification evidence

- `cargo test -p moleapi-core -p moleapi-server`: passed 28 tests, including 15 core tests and 13 server tests, on SQLite and isolated loopback servers.
- `cargo check -p moleapi-server --no-default-features`: passed; native library does not require web assets.
- `cargo fmt --package moleapi-core --package moleapi-server --check`: passed.
- `cargo clippy -p moleapi-core -p moleapi-server --all-targets --no-deps -- -D warnings`: passed for owned backend modules. A full dependency-inclusive run initially found six lints in root-owned format converters; root was notified to fix those.
- Standalone binary smoke test passed: dynamic loopback bind, embedded UI, health endpoint, first-user setup registration, persisted restart, setup-token suppression after setup, graceful SIGTERM shutdown, and database filename containing `?`, `#`, `%`.

Networking tests exercise real local HTTP servers for interpolation, headers/auth/query/body, JSON assertions, form delimiter escaping and encoded templates, redirect semantics and credential stripping, timeout behavior, binary responses, 5 MiB response truncation and fail-closed redirect errors. Router/storage tests cover repeated migration, first-admin concurrency, unique accounts, account isolation, CAS contention, snapshots, large JSON persistence, safe mock headers, redacted history, runner continuation and auth expiry. Native sync tests cover durable bases across router restarts, two-sided conflicts, explicit push/pull, snapshots, forbidden credential redirects and local edits while a remote request waits.

## Remaining verification and boundaries

Actual PostgreSQL/MySQL servers are not installed or configured in this workspace, so those engines have NOT been verified locally. The same real integration suite is ready to run against `MOLEAPI_TEST_DATABASE_URL`, `MOLEAPI_TEST_POSTGRES_URL`, and/or `MOLEAPI_TEST_MYSQL_URL`; CI must provide PostgreSQL 16/MySQL 8.4 services. The suite runs migrations twice, first-admin contention, account/session/workspace operations, tenant isolation, revision contention, snapshots and 90 KiB JSON payload persistence against every supplied URL. Compilation and dialect SQL generation do not substitute for those real runs.

Foundation scope excludes broader protocol clients, arbitrary scripting, team membership administration, database test-target connections and remaining research capabilities. Future work should retain these module boundaries and extend contract/schema through explicit versioned changes.

Root-owned converter review flag: canonical `specifications[].source` was initially retained unchanged by `include_secrets:false` redaction. Raw imported source can contain inline credentials; root was notified to scrub known credential source fields or explicitly warn before publishing a secret-free export claim.
