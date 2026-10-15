# Kotlin OkHttp request snippets

Parent: remaining-work-plan milestone2; code-generation.md and SNIPPET-COVERAGE.md.
Execute inline. Finish implementation/test code before final batch verification.

Goal: original Kotlin snippets compile on Kotlin2.4/OkHttp4.12 and send real JSON,
repeated form, text/control, binary/MIME and multipart requests, preserving literal
Kotlin dollar/interpolation text. No source rewriting or replacement client in QA.

- [x] Extend the trusted fixture example with kotlin mode and explicit profiles;
  preserve previous modes and source/hash manifests. Include dollar interpolation.
- [x] Add explicit JDK/Gradle/Node native wire driver with original snippet wrappers,
  pinned dependencies/locks, source/tool/dependency hashes and deterministic cleanup.
  Write the complete test program, then obtain actual baseline compiler/wire evidence.
- [x] Repair reproduced modern Kotlin API failures in the pinned mature emitter.
  Reuse OkHttp native string/file/form/multipart builders. Route raw files through
  the verified Scalar seam; scope repeated-form acceptance and GET/HEAD rejection.
- [x] Run all seven native profiles, existing Java/other adapter stability and wire
  regressions, generation/API/UI/i18n/build/lint/format gates and patch idempotence.
- [x] Apply ui-ux-pro-max and emil-design-eng for localized Kotlin prerequisites in
  the current dialog; preserve privacy, filename guidance, keyboard/theme behavior.
- [x] Record exact version/platform limits, commit/integrate/push verified work
  independently of pending Apple model/RPC verification.

Other versions/platforms/auth/transport/header/method/response behavior, other languages,
advanced SDK/schema work and all later milestones remain within the original objective.


## Execution ledger

- Original core compilation failed on MediaType.parse with Kotlin2.4/OkHttp4.12:
  /tmp/moleapi-kotlin-snippet-baseline.log. No old dependency was substituted to pass.
- Modern pinned emitter + explicit Scalar raw-file route pass all7 exact native
  profiles in /tmp/moleapi-kotlin-snippet-final, including literal interpolation.
  Client methods stay native; only imports/main/shutdown wrap original snippets.
- 54 other outputs stay byte-identical.8 Java file/multipart regressions pass after
  the shared checker extension; retained Gradle locks/JAR/tool/source/wire evidence.
- 92 generation/4 API/34 UI-i18n cases, frontend build/scoped strict Clippy/format and
  Python checks pass. A second pinned patch rebuild produces the same bundle bytes.
- Explicit GET/HEAD body rejection avoids the mature OkHttp get/head body omission.
  Default privacy and filename-only code generation remain independently asserted.

- Real owned-Rust-API UI harness (not mocked generation) verifies English/light1280px
  and Chinese/dark375px, modern file source, extension guidance, default credential/
  byte exclusion and no horizontal document overflow. Scoped axe has0 verified
  violations in both themes, with1 manual check each. Existing semantic gutter fix,
  focus controls and no new motion follow the requested two UI skills.

| Before | After | Why |
| --- | --- | --- |
| No Kotlin import prerequisites beside bare method-body code | Localized companion-extension/File guidance after generation | Make the native code usable without changing source or adding modal steps |
