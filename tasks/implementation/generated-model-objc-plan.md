# Native Objective-C model acceptance

Parent: remaining-work-plan milestone2; docs/specs/code-generation.md and
docs/MODEL-GENERATION-COVERAGE.md. Execute inline in the application worktree.

Goal: original Quicktype Objective-C models compile and roundtrip through actual
Apple Foundation under ARC, with honest enum/error/null evidence on Intel and ARM.
No generated-source rewrite or alternate model/JSON codec is permitted.

## Tasks and files

- [ ] Add tools/models/verify-objc.py, using original models.h/models.m, native
  QTPet/QTStatus construction, Unicode/zero/recursive/null assertions and original
  manifest/source/hash checks. Explicit xcrun only; reject non-Darwin hosts.
- [ ] Extend the existing optional build.yml Apple QA job to generate and run both
  OpenAPI3.0.3/3.1.0 model cases before protobuf RPC checks; upload actual evidence.
- [ ] Run actual compiler/runtime negatives for unknown enum, non-object and null
  NSError output. Diagnose observed failures before renderer changes. Reuse mature
  enum/model conversion and Foundation error handling in any pinned patch.
- [ ] After all implementation/test code, run native cases, unaffected-target byte
  comparisons, generation/API regressions, fmt/lint and idempotent engine build.
- [ ] Update coverage/catalog only for actual passed cases; commit/integrate/push
  verified slices while retaining pending Apple RPC changes until their gate passes.

## Review focus and limits

Check nested native objects rather than dictionaries; enums must not silently become
nil; optional nil and NSNull must not become invalid recursive objects; NSError** may
be NULL; native objects must remain alive under ARC. Integer range, required/constraint
validation, alias/union/container options, null/absence fidelity and other platforms
remain separate required parent work. No blanket schema or full module completion.
