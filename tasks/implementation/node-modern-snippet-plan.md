# Modern Node request bodies: Fetch, Axios, ofetch and Undici

Continue generation milestone2 using the pinned Scalar emitters and existing canonical HAR adapter. Legacy Native/Request/Unirest and browser-only variants retain their separate coverage.

- Extract the existing mature Node Fetch body setup as a shared Node helper, reused by native Axios/ofetch/Undici emitters; do not add a parallel application language emitter or wire encoder.
- Preserve raw JSON/text shape, Unicode/control bytes and MIME. Use Node fs for filename-only files, URLSearchParams for repeated forms, and form-data's native multipart encoder for ordered fields/files/explicit field MIME. Multipart bodies may materialize bytes; disclose the form-data dependency.
- Enable raw files and repeated forms for these verified variants. Keep WHATWG Fetch/ofetch GET/HEAD-body rejection; allow low-level Axios/Undici only after actual wire proof. Add explicit fixture rejection evidence and native API probes rather than silently dropping unsupported bodies.
- Write implementation and trusted verifier before batch tests. Cover4 clients×19 profiles (existing PHP18 plus HEAD binary), with original .mjs sources, explicit Node/dependency hashes, syntax checks and native Node HTTP observations. Expected4 Fetch/ofetch GET/HEAD rejections are separate from actual native requests.
- Pin current Axios1.20.0/ofetch1.5.1/Undici8.12.1/form-data4.0.6 in an isolated QA package/lock. Node/global Fetch is explicit consumer tooling; MoleAPI needs none of these at runtime.
- UI uses ui-ux-pro-max/emil-design-eng for contextual Node ESM/package/file hints with current typography/theme/focus/wrapping.
- After code/test completion, batch native/probe/old-output checks, generation/API/UI regressions, deterministic/clean patch checks, scoped lint/build/format and actual bilingual owned-API UI. Build frontend before Rust embeds assets; do not mutate embedded inputs during Rust compilation. Commit explicit files, cherry-pick verified changes to main, push both branches. Preserve drafts and pending Apple native CI. Full395-feature plan remains active.

## Evidence checkpoint

72 original-source syntax/native HTTP cases +4 direct native API/application rejections pass (4 clients×19 profiles). Node26.11.1/Linux with all declared current library versions and full lock. Native Axios/Undici GET/HEAD file bytes are preserved; Fetch/ofetch body rejection is proven, not silently skipped. Text MIME parts have no filename.10 prior defects reproduced and51 other representative outputs unchanged.97 generation/3 API/50 UI-i18n cases pass, plus strict scoped Clippy, fmt, frontend/server builds, deterministic engine and clean full upstream patch application. Old assertions rejecting now-supported Fetch repeated forms were updated after exact native wire proof; other unsupported guards remain. Actual owned-API en/light1280 Axios multipart and zh/dark375 Undici binary-filter UI preserve dependencies/privacy/reflow; inspected screenshots and scoped axe0 verified violations/1 manual contrast check. Evidence: `/tmp/moleapi-node-snippet-final-2` and `/tmp/moleapi-node-snippet-*.log`.

The full plan remains active; browser variants, legacy extended Node bodies, broader transport/auth/header/streaming/lifecycle/platform/dependency scopes and Apple native gates remain separate work.
