# Saved Mock implementation scripts

Spec: `docs/specs/mock-scripts.md`. The complete `mock-006`/`mock-012` matrix rows also require broader Mock work outside this narrow saved-example entrypoint.

1. [x] Model and privacy: add optional example script with v2-only workspace validation, old JSON compatibility, exact full backup, screened default backup and external-export rejection. Watch a failing regression first.
2. [x] Worker contract: add a bounded Mock response seed and `pm.mock` setters to v2; preserve the existing request phase output and `pm.datasets` bridge. Verify status/header/body bounds, request immutability, Promise settlement and missing/private dataset failures in a real worker process.
3. [x] Owner route: pass a bounded incoming GET snapshot into the worker, execute the saved script, construct the response through the existing safe header filter, reject owner/revision/cancellation changes and suppress private diagnostics. Keep static examples unchanged. Verify actual HTTP bytes and foreign owner denial.
4. [x] Product flow: add an accessible bilingual script editor to saved examples, preview the owner route, and check English/Chinese save/reload/execute at 390px without horizontal overflow.
5. [ ] Gates: focused Rust/frontend suites, strict Clippy/format/build, configured three-database CI, native package checks and independent review. Update coverage only with exact evidence.
