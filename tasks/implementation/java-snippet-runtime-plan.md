# Java request snippets and native file bodies

Parent: remaining-work-plan milestone2; code-generation.md and SNIPPET-COVERAGE.md.
Execute inline. Finish implementation and test code before running batch tests.

Goal: all four advertised Java HTTP adapters produce real usable requests across JSON,
URL-encoded form, text, raw binary and multipart file profiles. Enable file capability
only with actual original-source compile/call evidence. Product generation never reads
host files or executes the emitted code.

- [x] Extend the trusted fixture_snippets example with java mode and explicit profiles,
  retaining existing modes/defaults. Produce original snippet files and hash records.
- [x] Add tools/snippets/verify-java.py with explicit JDK/Maven, pinned library versions,
  native imports/main wrappers (no snippet rewrites), actual local HTTP observations,
  exact auth/query/header/body assertions, native client cleanup and tool/source hashes.
  Cover all4 libraries by6 profiles and preserve input/source bytes during compilation.
- [x] Extend only the pinned mature Java emitters' raw-file seam: native Files byte
  reads for nethttp/asynchttp/unirest and native RequestBody(File) for OkHttp. Reuse
  existing mature multipart/form handling. Record reproducible patch-package changes
  and enable Java binary_file metadata only after implementation/test code is ready.
- [x] Run24 actual compile/call cases, including Unicode/quotes/CRLF/control bytes,
  repeated form/multipart fields, non-UTF8 file bytes and disabled-part exclusion.
  Diagnose actual compiler/wire failures before fixes; never adjust expectations to
  accept dropped fields/body bytes or substitute a handwritten client.
- [x] Check mature patch rebuild idempotence, other target byte stability, generation/
  API regressions, scoped strict Clippy/format and prior snippet wire regressions.
- [x] Update precise coverage/options/dependency limits, commit/integrate/push verified
  work separately from pending Apple Objective-C runtime gates.

Retain explicit filename prerequisites/default privacy, original upstream licenses,
host-free generation and all parent constraints. Other Java library/runtime/version/
platform/transport/auth/method/body combinations and the full module stay open.


## Evidence and decisions

- 20 core/form/text/binary/multipart cases passed, then4 explicit binary-MIME cases
  proved the existing shared HAR projection already preserves the chosen MIME. No
  speculative MIME fix was applied. Final six-profile run additionally records exact
  JAR/tool/engine/source hashes. Evidence roots /tmp/moleapi-java-snippets-*.
- The locked @scalar/snippetz original tarball SHA512 matched. patch-package creation
  was blocked by npm's remote-URL policy (EALLOWREMOTE on locked npmmirror URL).
  npm pack with the registry version supplied the verified original; five standard
  Java diffs were appended to the existing reproducible package patch. No global
  npm configuration changed. Rebuild applies/idempotently reproduces exact bytes.
- 51 other representative adapter sources remain unchanged. Native form acceptance
  is limited to these four libraries; unsupported Java OkHttp GET/HEAD bodies now
  reject explicitly.91 generation/4 API/33 UI-i18n cases, strict scoped checks and
  seven earlier native wire cases pass.
- Actual UI harness uses the real owned Rust API, not mocked snippet replies; it
  confirms four Java file libraries, localized prerequisite guidance, default token/
  saved-byte exclusion and1280px English/light plus375px Chinese/dark layout.

| Before | After | Why |
| --- | --- | --- |
| Dark CodeMirror gutter #7d8799 on #282c34,3.86:1 | Existing request-dialog scope uses semantic gray-11 (rendered rgb176/180/186); scoped axe0 verified violations | Normal-size line numbers must remain readable; preserve theme/layout/focus |

Apple run38687292636 separately proves8 ARM source/descriptor RPCs but fails on the
Quicktype Objective-C unknown enum. Intel hit the60-minute job ceiling. Model repair
and a larger cold-build budget remain the next independent batch, not a Java claim.
