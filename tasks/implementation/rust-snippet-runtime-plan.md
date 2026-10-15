# Rust Reqwest native request bodies

Continue the full generation milestone, retaining the existing Scalar Reqwest emitter and moving its raw-file routing from the separate Postman fallback to that repaired mature emitter.

- Use Reqwest's native body/form/multipart builders with std::fs file reads. Preserve raw JSON/text, control escapes, repeated fields, enabled order, explicit field/file MIME and filename. Do not introduce a Rust emitter or MIME encoder in the application.
- Repair the existing Rust string literal helper for controls. Use raw JSON text in body() to avoid interpreting JSON Unicode escapes as Rust source. Files use Part::bytes with actual selected filename and mime_str propagation; no file bytes enter generated source.
- Allow repeated forms only after native proof; retain source immutability, default privacy and owner checks. File capability count remains31 because this target already advertised files.
- Write all implementation and trusted QA code before batch tests. Compile unchanged original snippets inside separate async consumers in one pinned Cargo QA project, then call native Node HTTP for21 profiles (Node19 plus JSON controls/Bidi/raw delimiter text and an empty form), including GET/HEAD file bodies. No imported user code executes.
- Pin current Reqwest0.13.5 and Tokio in an isolated Cargo manifest/lock with required form/multipart/TLS/runtime features. These are consumer/QA dependencies; do not upgrade the application's Reqwest or add a toolchain/runtime requirement to MoleAPI.
- Apply ui-ux-pro-max and emil-design-eng to post-generation async/Result/runtime/features/file hints using current type/theme/focus/reflow.
- Batch native compile/wire, previous-output failures, generation/API/UI regressions, clean/deterministic patch checks, scoped fmt/Clippy/build and actual bilingual owned-API UI. Build frontend assets before Rust server embedding; keep embedded inputs stable during compilation. Commit explicit files, cherry-pick verified slice to main and push both branches. Preserve drafts, existing Apple native CI and the full395-feature goal.

## Verified checkpoint

21 unchanged-source compile/native HTTP cases pass with the exact pinned Reqwest0.13.5/Tokio1.53.3 Cargo project on Rust1.99.0/Linux. The empty-form typed slice additionally fixes reproduced E0282;5 previous emitter failures were reproduced with real compiler/wire evidence.98 generation/3 API/51 UI-i18n cases, scoped strict Clippy/fmt, frontend/server builds and deterministic/clean upstream patch checks pass.54 other representative outputs are byte-identical. Original sources stay unchanged, and request/tool/compiler/lock/consumer hashes plus resolved dependencies and actual wire observations are preserved.

Owned-API browser checks on en/light1280 multipart and zh/dark375 binary verify native reads/parts, localized prerequisites/privacy/reflow. Screenshots inspected; scoped axe0 verified violations/1 manual contrast check each. Final form-only typing does not alter these two displayed branches. Evidence `/tmp/moleapi-rust-snippet-final-3` and `/tmp/moleapi-rust-snippet-*.log`. Broad parity and other milestone requirements stay open.

Apple audit: retry38718238486/Intel116366105458 completed failure13:05:06UTC. Original source RPC consumer passed at12:03:32; descriptor xcodebuild Release timed out after3600sec at13:04:25. Model validation did not execute. Full raw log `/tmp/moleapi-objc-retry-intel-38718238486.log` is preserved. No restart based on observation timeout; inspect/reuse the repeated native dependency build as the next Apple action while preserving all modes/representations.
