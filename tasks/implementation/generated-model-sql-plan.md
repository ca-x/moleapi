# Native generated SQL acceptance

Parent: remaining-work-plan milestone2; docs/specs/code-generation.md and
docs/MODEL-GENERATION-COVERAGE.md. Execute inline in the application worktree.

Goal: execute original PostgreSQL and MySQL DDL against real engines, retaining
exact source/artifact hashes and reporting database version and upstream limits.

- [x] Add tools/models/verify-sql.py and pinned QA-only Python dependencies. Consume
  trusted fixture_models outputs for3.0.3/3.1.0. An explicitly named URL environment
  variable supplies a QA administrator connection; never print credentials.
- [x] Create a fresh random database (no IF NOT EXISTS), execute original aggregate
  DDL without rewriting/splitting it, insert/query zero/enum/NULL/Unicode/JSON text,
  inspect native column types and reject unknown enum/missing required ID. Drop only
  the database this invocation created, in finally. Check generated hashes afterward.
- [x] Use isolated native PostgreSQL/MySQL services with explicit data/socket paths;
  no host package installation or modification of existing databases. MariaDB can
  provide separate compatibility evidence, not substitute for actual MySQL.
- [x] Finish implementation/test code before runtime gates; run both dialects and
  engines, generation/API/format/lint regressions, then update coverage/catalog and
  integrate verified work separately from pending Apple QA.

Review focus: actual server identity, native enum constraints, required/null semantics,
Unicode encoding, cleanup after rejection and unchanged original DDL. Nested Pet is
upstream TEXT, not a relational foreign key; SQL roundtrips do not prove model-object
or recursive schema fidelity. Other schemas/options/versions/platforms and advanced
generation/product/release work remain open.


## Execution record

- Both generated dialects pass actual PostgreSQL18.6 and MySQL Community8.4.11
  execution. Fixture/evidence roots: /tmp/moleapi-model-sql-3.0 and -3.1.
- Exact package signatures/hashes, dependency lock and private service records live
  under /tmp/moleapi-sql-qa-tools. No host packages or existing databases changed.
- Ruling: use native Oracle MySQL for the MySQL claim; separately downloaded MariaDB
  packages are not substituted. PostgreSQL's slow original mirror was supplemented
  with a faster official mirror copy matching the same signed package and hash.
- The database CI now performs the same consumers using its existing16/8.4 services;
  those versions remain pending until the corresponding CI result is observed.
- Scope retains upstream TEXT for nested properties and all parent-module limits.

- Final batch gates:80 generation cases and1 owned model API case passed; scoped
  strict Clippy, rustfmt, Python compile, actionlint and diff checks passed. Both
  dialects produce byte-identical aggregate DDL. Separate metadata queries confirm
  no owned QA database remains after all4 runs. Original emitted files stay unchanged.
