# Dart HTTP and Dio native request bodies

Parent: remaining-work-plan milestone2; code-generation.md and SNIPPET-COVERAGE.md.
Execute inline; finish implementation/test code before final batch verification.

Goal: both Dart HTTP libraries compile/analyze/AOT-run original emitted code for
JSON, repeated forms, text/control/dollar literals, raw files/explicit MIME and
multipart repeated fields/files. Product generation never reads files or executes code.

- [x] Extend the trusted fixture and native Node wire verifier for dart/http and
  dart/dio, with explicit Dart SDK/Node, fixed packages/locks, unchanged snippet
  bytes, original request and tool/dependency/engine hashes.
- [x] Obtain original runtime/compiler/wire evidence before repairing failing paths.
  Prefer pinned mature Postman native Request/MultipartRequest/FormData helpers over
  a parallel emitter. Preserve raw UTF8, repeated fields, filenames and MIME through
  native SDK/package APIs; reject unsupported configurations instead of dropping data.
- [x] Enable both file capabilities and repeated form acceptance only with complete
  native tests. Preserve private defaults and original authored request source.
- [x] Run both libraries by seven profiles, analyze/AOT compile/run, unaffected53
  adapter stability, pinned patch rebuild, generation/API regressions and scoped checks.
- [x] Apply ui-ux-pro-max and emil-design-eng to concise localized Dart prerequisites
  in the current dialog; verify real owned-API bilingual/theme/width/privacy behavior.
- [x] Update coverage/parent status, commit/integrate/push verified work separately
  from the still-running Apple repair gate. Other versions/platforms/options/auth/
  transport/response/language/schema/SDK and later milestones remain open.


## Execution ledger

- Original Scalar Dart/http core passed, then text produced an empty body without
  Content-Type: /tmp/moleapi-dart-snippet-baseline. Mature Postman route reuses native
  Request/MultipartRequest instead of another custom renderer. MIME projection is
  Dart-only; native repeated form/MIME/file/UTF8 helpers are pinned package patches.
- First repaired run exposed missing HAR text-MIME projection (fixed in the adapter).
  Next run reached multipart but QA's File( substring matched MultipartFile.fromFile,
  causing unused dart:io; the wrapper detector was corrected without snippet rewrites.
- All14 native analyzer/AOT/wire cases pass in /tmp/moleapi-dart-snippet-verified.
  Final packages are http1.6.0, Dio5.11.1 and http_parser4.1.2 on Dart3.13.5/Linux.
  Shared header/auth/query/bytes checks, source/tool/lock/AOT integrity are retained.
- 53 unaffected outputs remain byte-identical; idempotent build,93 generation/4 API/
  35 UI-i18n cases, strict scoped checks and7 prior actual native wire regressions pass.
  Foreign versions/platforms/options and all parent scope remain open.

- Real owned-Rust-API UI harness verifies both Dart file choices, native byte readers,
  async/package guidance, no saved-byte/token exposure,1280px English/light and375px
  Chinese/dark without document overflow. Both rendered scopes have0 verified axe
  violations and1 manual check. Source screenshots are in /tmp/moleapi-dart-snippet-*.

| Before | After | Why |
| --- | --- | --- |
| Dart file adapters hidden; no Dart async/dependency guidance | Two verified native file options and localized hints within the existing dialog | Preserve familiar focus/theme/privacy while making original code usable |
