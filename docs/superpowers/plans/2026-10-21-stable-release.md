# MoleAPI0.2.0 stable release

User authorization: push all completed code, merge it, and publish a usable stable version with Docker. Preserve unrelated root mise.toml and existing unrelated QA server.

1. Finish native hosted member regression/review and current-build QA. Remove only our temporary example.
2. Align Cargo/npm/Tauri0.2.0, remove Windows beta override, write bilingual notes and preserve explicit platform/EDAS/signing limits.
3. Run frontend796/build, affected backend39+policy, strict scoped Clippy, formatting, workflow lint and release assembly guards.
4. Commit/push intended files, preserve PR41 description, wait exact-head checks, mark ready and merge without bypassing checks.
5. Tag the merged main commit as v0.2.0; wait exact-tag Application/native/Docker checks. No local Docker build.
6. Download14 release assets, verify SHA256SUMS and exact archive/installer matrix, read anonymous GHCR dual-architecture manifest, then publish draft as stable/latest.
7. Read back release/registry/main/tag/local state and report exact delivery plus remaining product scope.

Final source review: native_shared_final_review rechecked all prior findings and bounded JSON-text bearer regression, then separately reviewed stable packaging/version/assets changes with no blockers. Installed Tauri GUI and signing remain explicit limitations; CI must prove native build and Linux installed-package resource smoke for the release tag.
