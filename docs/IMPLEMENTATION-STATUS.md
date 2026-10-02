# MoleAPI development status

Development is in progress against the full capability matrix; this is not a claim of complete Apifox/Postman parity.

## Foundation implemented and verified locally

- Modular Rust request engine, network policy and redirect checks, bounded interpolation, response caps, declarations/assertions and redacted history.
- SeaORM-backed auth/setup/session, workspaces/revisions/snapshots, Mock examples, collection runner and native optional cloud synchronization. SQLite tested; PostgreSQL/MySQL use the same implementation and await real CI containers.
- Source-preserving OpenAPI3.0(openapiv3)/3.1(oas3), official-schema-validated Postman2.1(jsonschema), supported cURL and MoleAPI import/export. Canonical source is preserved, with structured credential redaction by default; includes-secrets export is explicit. Unsupported source configuration is reported, not silently promised executable.
- Functional React feature modules, Radix components, CodeMirror, TanStack Query, real request/response/history/runner/import/export/environment/sync controls. Resizable request/response split uses react-resizable-panels. Save concurrency, CAS conflict recovery and account-scoped state have regression tests.
- Actual hosted-binary UI has been tested with agent-browser at1280x640/1360x768/360x640/390x640. Real local healthGET and public EchoPOST return200JSON; assertions pass. Keyboard resize and no viewport horizontal overflow verified; automated light/dark accessibility checks reported no violations (manual completeness is not claimed).
- All assets are embedded in server binaries, including debug builds; per-file SHA256 bundle digest is a compiler input and servedindex matches currentdist. Production missingdist failsbuild.
- Tauri local IPC adapter/config, offlineSQLite path, user-provided icons, Docker/compose/environment configuration and Actions matrices implemented; actual installer/image builds are pending CI validation.

Local verification:41 Rust tests,17 frontend tests, rustfmt, Clippy all-targets -Dwarnings, TypeScript and frontend productionbuild passed. Numbers will grow with subsequent modules; actual PG/MySQL/native package verification must be reported separately.

## Continuing work

The full matrix remains the authority. Next work extends variable scopes/local overrides, JS pre/post scripts and pm APIs, richer authentication/body modes, protocol sessions and schema editors; then scenario/dataset/Mock/publishing, collaboration/versioning/Git/CLI/AI/enterprise modules. Vendor-marketplace/billing entries retain explicit product-boundary distinctions. No unimplemented feature is labeled complete merely because a component or route exists.
