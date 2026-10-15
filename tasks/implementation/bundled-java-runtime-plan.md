# Bundled Java runtime distribution

Parent: remaining-work-plan milestone2, code-generation.md and BUILD-AND-RELEASE.md.
Execute inline in the application worktree. Finish implementation and tests before
running batch tests. Existing untracked task drafts remain untouched.

Goal: official standalone archives, desktop packages and both container architectures
carry a fixed verified Java runtime usable without PATH configuration or downloads
at application runtime. Explicit environment and native-user selections take priority.

## Files and steps

- [x] Pin Eclipse Temurin JRE21.0.12.1+1 assets for Linux x64/arm64, macOS x64/arm64
  and Windows x64 in vendor/java-runtime/manifest.json, using official release SHA256s,
  exact archive sizes/layouts and retained GPLv2/Classpath-exception legal files.
- [x] Add scripts/prepare_java_runtime.py: explicit target or build-host selection,
  capped HTTPS archive download, exact hash/size checks, bounded safe stdlib extraction,
  regular-file payload/inventory and versioned receipt. Preserve all runtime file
  contents; materialize internal aliases for portable resource-copy semantics.
  Generate an ignored desktop resource-map override, not product runtime fetch logic.
- [x] Add the shared Rust known-resource-path resolver. Accept only this compiled
  platform's pinned receipt and contained executable. Environment wins; absent bundles
  keep explicit dependency state. Desktop loads its Tauri resource directory before
  restoring its existing native-user preference. No PATH/external-spec discovery.
- [x] Ship runtime/java alongside server archive binaries; pass the generated resource
  config into all four desktop package jobs and the default npm desktop build command.
  Enforce correct prepared target/resource mapping for desktop release builds.
  Include the same pinned JRE in both Docker architecture images at build time.
- [x] Add real packaged-server smoke tooling: clear external Java configuration,
  check bundled Java, start the actual server, register an isolated owner, create
  canonical spec, generate Go model code through the normal API, confirm source and
  engine, and clean up. CI runs it from each staged standalone distribution; containers
  check the bundled JVM and real generation separately from HTTP/worker smoke.
- [x] Write extraction/hash/receipt/platform/traversal/cache tests and Rust resolver/
  precedence/containment tests. After code/test completion, run all5 original archive
  integrity/layout cases, actual local Java/server/desktop/container gates where tools
  exist, generation/API regressions, formats/Clippy/actionlint and builds.
- [x] Document exact platform evidence and licenses, update parent checkpoint, then
  integrate verified work independently of still-pending Apple protobuf/model QA.

Review focus: do not package the build host's JRE for a cross target; retain original
legal files; do not execute an imported specification or download at runtime; unknown/
tampered receipts must not choose ambient Java; failed preparations retain prior valid
payload; user selections continue to work. macOS signing may legitimately change native
binary signature bytes, so runtime discovery validates pinned receipt/layout rather
than asserting original pre-signing executable SHA as a code-signing replacement.

Other OS/toolchain/library/advanced generation, final installer/signing/notarization
and full later milestones remain open until actual acceptance evidence exists.


## Execution evidence

- All5 official archives pass pinned SHA256/size plus actual extraction, full legal
  tree/inventory and original release-file OS/architecture/version checks. Linux
  payload is restored after cross-target preparation. Evidence under
  /tmp/moleapi-bundled-java-platform-layouts; Git-ignored inputs remain local.
- Actual staged Linux server passes bundled JVM and authenticated owned3.1 generation:
  17 Go files, exact original source and manifest, with no external Java and invalid
  PATH. Native Rust fixture passes real bundled generation with injected invalid
  JAVA_TOOL_OPTIONS cleared. Real Linux native production-asset UI with a fresh profile
  and no Java override/preference generates45 C# files using the included runtime.
- 8 Python tests and14 API cases pass. Generation suite passes with archive/receipt/
  platform/traversal/cache guards; final containment test additionally rejects an
  entire payload directory pointing outside owned resources. Scoped strict Clippy,
  format, Python compilation/actionlint and native build/resource/JVM gates pass.
- Other native OS execution and new Docker pipeline remain unclaimed until CI results.
  Existing Apple Objective-C run38687292636 is now active on both architectures; its
  old ref is independent of this Java batch and must not be restarted on timeout.

- Final guards reject mismatched release target and missing resource-map configuration
  by executing the actual compiled desktop build script (not a duplicated validator).
  Evidence: /tmp/moleapi-bundled-java-release-guards.json.88 generation cases pass,
  including payload-directory escape rejection, with final strict Clippy passing.
- All5 layouts and real Linux native/server cases are proven; foreign-OS execution,
  new container image execution, installed-package/signing/source publication and all
  remaining generation/later milestones stay open. No blanket release claim is made.
