# Python request snippet runtime plan

Scope: Requests, HTTPX sync and HTTPX async using the existing pinned Scalar requestsLike emitter. Stdlib http.client and aiohttp are a subsequent slice; their catalog presence is not native runtime evidence.

- Preserve saved source, credential opt-in and filename-only generation; never read user files in the app.
- Repair the existing JSON literal conversion without changing quoted string contents, including root false/null.
- Emit UTF-8 bytes for raw text and native file reads for raw binary. HTTPX uses content=; forms retain native data=/files=.
- Allow repeated form fields only for these verified libraries. Keep other adapters unchanged.
- Write a trusted native HTTP verifier and fixture profiles before running the batch. Use isolated pinned QA dependencies, original generated source, syntax checks and observed auth/query/header/body/file bytes. Include lexical traps, top-level false/null, empty JSON strings, forms and multipart repeats.
- After implementation and test code, run the batch, compare upstream behavior for demonstrated failures, verify non-Python output parity, generation/API regressions, formatting and Clippy.
- Record exact evidence and limitations, commit explicit files, cherry-pick verified slice to main, push both branches. Apple changes remain pending native CI.

## Evidence checkpoint

Implementation and tests written first, then batched verification:39 native wire/syntax profiles,95 generation cases,3 snippet API tests (ownership/MIME and new Python file routes),39 UI/i18n tests, strict scoped Clippy, per-package formatting and frontend/server builds pass. Upstream contrast reproduces all9 JSON/text failures. Engine rebuild is deterministic;50 non-Python representative outputs are byte-identical. Real owned-API en/light1280 and zh/dark375 screenshots/axe verify guidance/privacy/reflow;0 verified violations,1 manual line-number contrast check. Artifacts: `/tmp/moleapi-python-snippet-final-4` and `/tmp/moleapi-python-*.log`.

The generic HTTPX request path also handles body methods without shortcut arguments; GET binary and DELETE JSON have actual native wire checks. Stdlib/aiohttp extended profiles and Apple CI remain separate unfinished work.
