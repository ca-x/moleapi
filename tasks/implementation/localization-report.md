# Localization implementation report — 2026-10-04

Status: frontend changes frozen for root integration/review; no commit created. Base: cb26e6c, branch feat/application, isolated application worktree. The parent feature goal remains active.

## Delivered interfaces and behavior

`web/src/shared/i18n/index.ts` exports `t(key, values?)`, `useLanguage(): { language, setLanguage }`, `setLanguage(language)`, `i18n`, `Language`, `LANGUAGE_STORAGE_KEY`, `message(key, values?)`, `translateCopy(copy)`, and `liveTranslation(key, values?)`.

Call `useLanguage()` in a rendered component/hook to subscribe. Use `t` for immediate text, `message` for app-owned copy retained in an open confirmation, and `liveTranslation` for Sonner content that should update while visible. Plain strings given to `translateCopy` remain exact; this is deliberate for external/user text.

Languages are `zh-CN` and `en`; local preference key is `moleapi_language`. Supported saved choice wins; otherwise a Chinese browser locale selects zh-CN, all other locales select English. Denied localStorage reads/writes fall back to browser detection/in-memory switching. Document language/title track selection. Auth/settings selectors use existing Radix components and autonyms 简体中文/English. The existing appearance preference also handles denied localStorage.

Initialization and selection changes use best-effort Tauri `set_language({ language })`, with a stale-language check after the async import. Missing bridge/tray does not reject UI switching. Root owns the native command and tray implementation.

The AST migration covered the 741 distinct original Chinese source strings, including protocol screens, dialogs, validation, navigation, accessible labels, toasts, samples created by the app, and workbench UI. Interpolation/plurals cover dynamic names/counts. Existing requests, descriptions, scripts, definitions, URLs, variables, responses, generated code, and third-party/server errors are untouched. New sample names/descriptions are localized only at creation. Catalogs deliberately disable key and namespace separators. Main catalogs currently have 939 matching keys each, including root's new Webhook/common keys.

## Editor integration and provenance

GraphiQL 5.4.0, @graphiql/react 0.39.1, and doc-explorer/history 0.4.4 have no shared locale option. Maintained patch-package changes route their curated fixed UI copy through the same react-i18next instance. A small adapter invalidates compiled render caches by language; subscriptions are added only to React components, never conditional compiler render helpers. The outer provider wrapper keeps its native cache, preserving plugin visibility. Documentation section titles retain original internal icon-map keys; only rendered headings translate. Query/schema/response data are not passed through the copy translator.

Monaco 0.52.2 has 1,719 embedded Chinese NLS messages extracted with Babel from its official shipped Chinese table and the matching numeric English messages in the same build. One genuinely translated missing upstream label, “Trigger Inline Edit”, is added locally. The namespace contains 1,720 entries. The patch uses i18next for NLS, localizes native command/menu metadata, updates explicitly owned widget label options on language events, and refreshes already-open command palette labels/a11y. It does not walk rendered DOM or replace arbitrary page/document text. Find/Replace fields, buttons, toggles, counts, and tooltips switch without changing text models. GraphQL editor aria labels use public `updateOptions`.

CodeMirror uses its native phrases facet and compartments. Stable callbacks/basic setup prevent a language switch from replacing its EditorView. A scoped @codemirror/search 6.7.2 patch updates the search panel's own captions/accessible labels and commits input events; document and search values are retained. Upstream CodeMirror intentionally redraws panels when phrases change: its Find input DOM identity changes while the equivalent field remains focused and the search draft survives. EditorView/document identity remains unchanged.

Original upstream operation-selection and tab accessibility patches are preserved. Six patches apply to pristine package files. npm 12 rejected patch-package's temporary install of remote tarball specifications (`EALLOWREMOTE`); no npm security setting was changed. Standard patches were captured using git diffs from local cached npm tarballs verified against each lockfile SHA-512 integrity. `tools/localization/capture-patches.py` and `verify-patches.py` document/reproduce that fallback. No runtime downloads or translation service are used.

## Verification evidence

- `npm --prefix web run test`: 36 test files, 128 tests passed, including the original 98 tests, 29 focused localization/error-state tests, and root's current Webhook test.
- `npm --prefix web run typecheck`: passed.
- `npm --prefix web run build`: passed; final production build after the error-state fixes completed in 23.97 s. Existing upstream “use no memo” Rollup messages and large GraphQL/Monaco chunk warnings remain non-fatal.
- `python3 tools/localization/verify-patches.py /tmp/moleapi-localization-pristine`: all six patches applied successfully to integrity-verified pristine package files.
- `git diff --check -- web tools/localization`: passed.
- Node 26's undefined Web Storage global interferes with Vitest 3/jsdom; test setup supplies genuine jsdom Storage objects. The existing tests explicitly start in Chinese, and locale tests switch themselves. No assertion was weakened.

Focused tests verify saved preference precedence, unsupported preference detection, denied storage initialization/writes, document metadata, native IPC/failure fallback, interpolation/plurals, complete genuine English catalogs, AST reference coverage/no bare Chinese UI, actual auth selector/focus/credentials, unchanged server errors, request toolbar/tabs and exact user fields, navigation, pending interpolated guards, visible toast content, GraphiQL documentation icon keys, and Monaco repeat-switch labels/templates/listener disposal.

Compiled browser QA used agent-browser session `moleapi-localization` against an isolated synthetic fixture, not documentation screenshots or the real hosted backend. It confirmed en→zh-CN→en controls, open documentation visibility, selected operation Second, unchanged request JSON, the same three Monaco editor IDs and model objects/content, original Chinese schema/document/variable data, already-open palette labels/a11y, live Find/Replace, and CodeMirror EditorView/document/search values with equivalent focused Find control. The fixture files now live only in `tools/localization/fixtures`; no test harness remains in web/src or the production build. Its isolated browser and servers were stopped. Root owns actual App screenshots and real hosted/native integration QA.

## Exact remaining acceptance limits

- Root still needs full actual App screenshots/QA for equivalent bilingual protocol, environment, settings/auth, narrow layouts and dark/light screens; synthetic editor checks do not replace these.
- Native menu propagation has frontend IPC test evidence only here; worker did not build/run native Rust. Root owns native source/CI/runtime evidence.
- Advanced Monaco widgets beyond tested workbench, documentation, palette and Find/Replace were not exhaustively exercised. Their official NLS is embedded, but this report does not claim every hidden editor workflow was browser-tested.
- The previously reported stored-local-error gap is closed by the follow-up below. External/server errors, completed responses, and parser-library diagnostics deliberately retain their original text. No known source-key snapshot remains in the existing app-owned UI error states covered by this migration.
- Independent task/final review is still pending: root's reviewer spawn/reactivation was rejected by the actual thread limit. Root is doing an inline source review, which is not reported as an independent review.

## Owned paths and coordination

Worker owns existing `web/src/App.tsx`, `main.tsx`, shared UI/model/api/readBoundedTextFiles, existing feature UI/hooks/helpers, auth/settings preferences/selectors, new `web/src/shared/i18n/*`, test plumbing, `web/package.json`, `package-lock.json`, `vite.config.ts`, dependency patches, and `tools/localization/*`. Root concurrently added Webhook feature files/tests/styles and integration edits in navigation/WorkbenchMain/catalogs; those shared files contain both parties' work. Root owns all Rust, README variants, screenshots, original untracked tasks/scaffolds, staging and commits. Worker did not stage files, commit, run local Docker, edit Rust/README, or remove unrelated work.


## Follow-up: retained error metadata (frozen after fresh verification)

`web/src/shared/i18n/errors.ts` provides `AppError`, `LocalizedError`, `ErrorCopy`, `errorCopy`, and `liveError`. Only explicitly created AppError instances carrying a LocalizedCopy can translate. Ordinary Error/string messages remain literal even when they exactly equal an app translation key. This uses explicit provenance, with no reverse translation, string registry, or guessed message matching.

GenerationDialog keeps the catalog-load failure as a source-key descriptor. GraphQL schema loading/introspection/save failures retain LocalizedError keys through catches; server-provided errors take a separate ordinary Error path. The same retained-copy pattern covers A2A/MCP/Proto/SOAP source dialogs and schema hooks, local definition-file/config validation, HTTP/JSON-response fallbacks, existing auth/request/session/modal error states, and error toasts. Hook APIs that previously exposed error strings still expose current translated strings derived at render. Raw server/target statuses are preserved. Root independently adopted the helper in new Webhook files; worker did not modify those files.

Twelve additional regressions verify Generation catalog failure switching without refetch/request mutation, exact upstream generation errors, GraphQL schema save/missing-schema errors without retry or draft changes, a server schema error equal to a local key remaining unchanged, MCP invalid-config draft retention, gRPC caught save-failure switching without repeating reflection, local interpolation/toast content, literal foreign errors, caught HTTP status/invalid-JSON fallback provenance, and unchanged actual server HTTP errors. The AST catalog-reference assertion recognizes LocalizedError constructor keys while retaining its catalog-completeness and bare-copy checks.

Fresh commands after the last code change: full frontend suite 36/128 passed, typecheck passed, build passed (23.97 s), and diff whitespace check passed. No dependency patch changed in this follow-up; root also freshly verified all six pristine-package applications. The earlier error-state acceptance warning is superseded by this section; actual App/native/independent-review evidence remains scoped as above.
