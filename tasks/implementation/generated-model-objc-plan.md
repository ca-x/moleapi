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
  OpenAPI3.0.3/3.1.0 model cases after protobuf RPC checks; upload actual evidence.
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


## Observed Apple failure and repair

- Run38687292636 ARM job116264955353 compiled and passed actual source/descriptor
  protobuf clients in all4 RPC modes (8 calls). Original generated source hashes
  match across representations; downloaded runtime-evidence.json proves the result.
- Its subsequent original Quicktype model executable decoded/constructed recursive
  zero/null/Unicode records, then aborted: Unknown enum silently accepted. The mature
  renderer's JSON conversion calls nullable withValue:, which returns nil on an
  unknown tag without reporting a decode failure. Public lookup remains nullable;
  a private JSON-only enum conversion now rejects non-null unknown values through
  the existing exception-to-NSError boundary, including nested conversions.
- Original serialization dereferenced NSError** even when NULL, and reused stale
  caller errors. Shared mature try/catch setup now supplies autoreleasing local
  storage, resets the selected output slot, and declares optional error pointers.
  Generated local names avoid the model parameter localError collision.
- Intel job116264955616 started06:51:05UTC and was cancelled07:51:49UTC while compiling/
  calling RPCs. Existing60-minute budget expired; native job budget becomes90 minutes.
  No consumer/profile/representation is removed to make the gate easier.
- Native QA now also requires public nullable lookup, nested-enum rejection, NULL-error
  failure paths and stale-error reset. These Apple outcomes remain pending until the
  repaired emitted source actually compiles/runs; local rendering is not Apple proof.

- Local candidate gates:91 generation cases/2 model API cases, strict generation
  Clippy, Rustfmt/Python/actionlint, idempotent engine build,17 unaffected target
  byte comparisons and5 prior native model runtime regressions pass. Actual Clang
  ARC syntax-only preflight confirms the new autoreleasing error-pointer setup;
  no Linux stub is counted as Foundation execution. Repaired Apple gate is pending.
