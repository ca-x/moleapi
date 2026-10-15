# Pinned build-time Java runtime

Official Eclipse Temurin JRE21.0.12.1+1 assets are locked in manifest.json for the
five supported standalone targets. The official release API supplied the exact
archive sizes and SHA256s; the corresponding OpenJDK source archive and source/build
revisions are recorded alongside the binaries. Runtime/JAR/model generation never
downloads or resolves an executable through PATH.

Prepare a native build (Python3.12+):

```sh
python scripts/prepare_java_runtime.py
```

Cross builds must name the actual supported release target:

```sh
python scripts/prepare_java_runtime.py --target aarch64-apple-darwin
```

An explicitly supplied --archive may reuse an already downloaded original asset;
its exact size/SHA256 remain mandatory. Downloads and current payloads are ignored
by Git and excluded from Docker build context. Preparation validates archive paths,
types/links and extraction budgets before replacing a previous valid payload.
Internal aliases become regular files with unchanged contents for consistent Tauri,
Windows ZIP and standalone archive copying. Inventory verification catches cache
modification before reuse. Never prepare two targets concurrently in one checkout.

The generated current/ directory contains the original distribution under payload/,
its full legal/ tree (Contents/Home/legal on macOS), a versioned platform receipt,
per-file hashes and corresponding-source.json. The primary OpenJDK license is GPLv2
with the Classpath exception; retain every upstream component license/notice. Source
metadata identifies the matching upstream release archive, not an invented license
or source offer. Final publication/source distribution and signing follow release gates.

Standalone distribution layout:

```text
moleapi-server[.exe]
runtime/java/moleapi-java-runtime.json
runtime/java/files.sha256.json
runtime/java/corresponding-source.json
runtime/java/payload/...
```

Desktop packages use the same runtime/java resource layout through the generated
desktop/tauri.java.json override. Normal native builds use npm --prefix web run
desktop:build; custom cross builds prepare both pinned engines, then pass
--config tauri.java.json and the matching --target to Tauri from desktop/.
Desktop release builds reject missing resources and a mismatched prepared target.
Docker uses this same pinned payload from a Python build stage, not a runtime download.

Explicit MOLEAPI_CODEGEN_JAVA wins over bundled resources. Desktop user preferences
restore afterward and retain their existing priority. Missing bundles preserve the
dependency state and native picker. Receipts validate known platform/version/source
hash and contained paths; they do not replace OS code signing or claim that post-signing
Mach-O bytes must retain upstream signatures. Verify actual packaging/runtime evidence
in docs/PROJECT-GENERATION-COVERAGE.md before making platform support claims.
