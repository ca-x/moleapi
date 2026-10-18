# Mock data binding implementation plan

Goal: let one selected Mock example return rows from a saved dataset view without authored JavaScript.

Spec: `docs/specs/mock-data-binding.md`. Authority: `mock-012` in `docs/FEATURE-MATRIX.md`; existing dataset, Mock script and public sharing contracts.

## 1. Core contract and compatibility

- [x] Add a failing old/new Example roundtrip and workspace validation test for binding references, source/shape/parameter bounds, mutual exclusion with schemas and v2 mode.
- [x] Add a typed optional `mock_data` value with absent-field backward compatibility; validate saved dataset/view ownership and JSON media type.
- [x] Update default/full backup and external interchange handling so bindings cannot silently disappear or expose private dataset material.

## 2. Selected Mock execution

- [x] Add real direct and routed Mock fixtures for literal and query-backed saved datasets, parameterized saved views, query/path/header bindings, result shapes and selected-only access.
- [x] Resolve parameters from bounded safe incoming fields, reject missing/duplicate/private values, and execute only the saved SQL view through the existing bounded dataset worker and live Data bridge.
- [x] Put data output into the Mock response draft before an authored script; retain status/header filtering, revision and share fences, cancellation, failure privacy and call-log metadata.
- [x] Add public grant, owner denial, source edit/deletion, simulated failure/rate-limit, empty result and output-limit regressions. A selected delayed binding cancellation also releases its slot and does not replay after abort.

## 3. Owner UI and acceptance

- [x] Add bilingual dataset/view/parameter/shape controls in the saved example editor, with stale-reference feedback and identity fencing.
- [x] Run focused and full frontend tests, production build, focused Rust tests, strict Clippy and embedded server build.
- [x] Exercise an actual 390px English/Chinese saved/reloaded owner route and public share. Record exact supported behavior and unresolved matrix work in a coverage file.
- [ ] Complete independent review, fixed-head Application CI with PostgreSQL/MySQL/SQLite, Docker and native package checks; merge a verified batch.
