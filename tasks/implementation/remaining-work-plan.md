# Remaining MoleAPI Development Plan

> **For agentic workers:** Execute one independently testable slice at a time. Read the linked module spec and coverage file before implementation; do not treat a catalog entry as feature support.

**Goal:** Complete the remaining functional scope, then perform unified review, cross-platform/database validation and release.

**Architecture:** Keep the Rust core/server/CLI and Tauri offline client on shared contracts. Extend existing module engines rather than introducing parallel parsers or runtimes. Use `docs/FEATURE-MATRIX.md` and `docs/CAPABILITY-MAP.md` as the row-level authority; this plan orders work but does not reclassify matrix rows.

**Tech Stack:** Rust/Axum/SeaORM, React/TypeScript, Tauri, existing pinned generator/protocol engines.

**Spec:** `docs/INITIAL-SPEC.md`, `docs/FEATURE-MATRIX.md`, module specs under `docs/specs/`.

## Ground rules

- Status is **open** unless the relevant coverage file has real endpoint/UI/CLI/native evidence and regression tests. A prior run only proves its exact commit and platform.
- Preserve offline desktop and standalone hosted server behavior; no hosted third-party service or runtime download as an implicit dependency.
- Protect source fidelity, owner boundaries, credentials, cancellation and resource budgets in every slice.
- Keep the existing untracked draft files untouched. No merge, push, release or claim of parity before the final gates.
- Recheck the feature matrix after each milestone and record supported target combinations separately from catalog availability.

## Ordered milestones

| Order | Deliverable | Acceptance gate | Authority |
| --- | --- | --- | --- |
| 1 — runner governance | Finish CLI label management; add workspace-scoped agent credentials with revocation; then label/group target selection with deterministic no-fallback dispatch, response-history upload, native hosted management and supervised service installation. | Cross-instance ownership/revocation/cancellation/lease tests, CLI process fixture, browser and native-platform evidence. Labels alone do not provide group scheduling. | `docs/specs/self-hosted-runners.md`, `docs/SELF-HOSTED-RUNNER-COVERAGE.md` |
| 2 — generation | Validate mainstream advertised SDK/server targets by compiling/starting and calling real fixtures; close OpenAPI 3.1/ref/union/auth/file, CLI raw/upgrade, protobuf language plugins, native picker and bundled Java/runtime gaps. | A per-target evidence table with exact generator versions and compile/run results; no unverified target advertised as supported. | `docs/specs/code-generation.md`, `docs/PROJECT-GENERATION-COVERAGE.md` |
| 3 — request/protocol completeness | Close remaining auth, cookies and network controls; implement remaining protocol rows (including Dubbo); finish Data integration scripts/extraction/history/source formats and import/export mappings. | Real wire fixtures and source roundtrips, privacy/limits, desktop/hosted parity, explicit unsupported cases. | `docs/FEATURE-MATRIX.md`, `docs/PROTOCOL-COVERAGE.md`, `docs/IMPLEMENTATION-STATUS.md` |
| 4 — product workflows | Complete scenario/testing, Mock, documentation/publishing, collaboration/versioning/Git, integrations and governance rows. Treat AI and vendor-marketplace/billing boundaries as separate decisions, not implicit parity. | End-to-end owner/role/restore/conflict tests and rendered user journeys for each implemented matrix row. | `docs/CAPABILITY-MAP.md`, `docs/FEATURE-MATRIX.md` |
| 5 — release gate | Audit every matrix row and coverage claim, independent security/functional review, three-database CI, native desktop/installers, embedded server and Docker smoke, then merge/release. | No critical findings, reproducible build artifacts, actual package installation/smoke and documented gaps. | `docs/BUILD-AND-RELEASE.md`, `docs/IMPLEMENTATION-STATUS.md` |

Milestones 1–4 each require a separate decision-complete slice plan before coding; the matrix is too large to safely implement as one undifferentiated task. The first slice is CLI label management, which closes the gap left by the existing hosted UI/API label work without changing scheduling semantics.

**Checkpoint:** Milestone 1 is in progress. CLI label management is implemented and verified in this worktree; the next security-sensitive slice is workspace-scoped agent credentials. Write its threat model and decision-complete spec before changing credential issuance or claim authorization.

## Slice 1: CLI runner labels

**Files:** `crates/cli/src/args.rs` owns flags; `crates/cli/src/runners.rs` owns request payloads; `crates/cli/tests/runners.rs` owns real process/API fixtures; `docs/SELF-HOSTED-RUNNERS.md`, `docs/SELF-HOSTED-RUNNER-COVERAGE.md` and `docs/IMPLEMENTATION-STATUS.md` own usage/status.

**Contract:** `runner register --label VALUE` repeats up to ten times. `runner set --label VALUE` replaces labels when present; `runner set --clear-labels` explicitly clears them; omitting both leaves labels unchanged. The two set flags conflict. Server validation is authoritative, and CLI behavior remains backward-compatible with old invocations.

- [x] Write a real hosted CLI-process regression: register labels, list, replace, omit-preserve, clear, duplicate/invalid rejection and unchanged revision on rejection. It failed first because `--label` was unknown.
- [x] Add Clap flags and request payloads without modifying queue/agent behavior. Reuse server validation; do not silently normalize or deduplicate user input.
- [x] Run the three CLI runner process tests and focused server label test, rustfmt, diff check and server/CLI build. Update usage, coverage and status. Strict CLI Clippy remains blocked by three pre-existing warnings in `Command::Run`, `tokens.rs` and `variables.rs`; all-targets Clippy passes when only those three existing lint categories are allowed.
- [x] Inspect the diff for accidental changes and leave pre-existing drafts, commit history and remote branches untouched.
