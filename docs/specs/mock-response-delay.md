# Bounded saved Mock response delay

Authority: `mock-007` and the delay part of `mock-013` in `docs/FEATURE-MATRIX.md`, the owner-scoped saved-example and matching routes, and `docs/specs/mock-scripts.md`.

## Contract

- A saved response example gains an optional `delay_ms` field, default 0 for all existing workspaces. Valid values are integer milliseconds 0–30,000. The setting survives full and default MoleAPI backups; external formats reject it because they cannot preserve response latency behavior.
- Both direct example preview and routed Mock dispatch select and construct the response first, then wait `delay_ms` before returning it. A dynamic Mock script still runs under its existing worker deadline; the configured delay starts after script completion and cannot be extended by script output. Logs record elapsed time including the delay.
- At most 32 delayed Mock responses may wait concurrently per server. Saturation returns 429 without sleeping or serving the configured body. Dropping the HTTP future releases the permit and cancels the wait. After the wait, recheck owner/workspace revision before serving static or dynamic content; changed/deleted workspaces fail closed. The existing no-delay path keeps its response behavior and does not consume a delay slot.
- The bilingual example editor exposes a labeled numeric delay field with millisecond unit and its 30,000 ms bound. The saved value is visible after reload at 390px. No client-side wait is simulated; the actual response time is controlled by the server.

## Evidence

- Core old JSON/default/full backup and invalid delay tests. Actual local/hosted HTTP timings for static and dynamic Mock, no-delay parity, 33+ concurrent admissions, revision/deletion during wait, owner isolation and log elapsed time.
- English/Chinese 390px save/reload/preview and real delayed route checks, focused Rust/frontend suites, strict Clippy/format/build, configured three-database CI and current-head native package checks.

## Remaining Mock scope

Schema-generated response bodies, public Mock permissions, richer failure scenarios and traffic inspection remain separate rows. A configured HTTP 4xx/5xx example or script can already supply failure responses; delay does not imply random fault injection.
