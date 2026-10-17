# HAR export implementation

1. [x] Shared bounded converter: add HAR 1.2 request-template export, explicit representability checks, warnings and privacy/roundtrip regressions.
2. [x] Product surface: expose `har` in the existing export picker, keep hosted/CLI on the shared converter, and cover owner/default/full behavior with real routes and a CLI process.
3. [ ] Acceptance: run focused Rust/frontend suites, strict Clippy/format/build, bilingual 390px browser export/reimport, three-database CI and relevant platform checks; update coverage with exact evidence.
