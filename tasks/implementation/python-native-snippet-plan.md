# Python http.client and aiohttp body completion

Continue the existing Python runtime slice using the pinned Scalar emitters and canonical saved-request adapter.

- http.client raw text must stay byte-for-byte UTF-8, including text that happens to parse as JSON. Native pathlib reads raw files; stdlib json and urllib.parse retain JSON/repeated forms.
- Replace its handwritten multipart string assembly with urllib3.filepost.encode_multipart_formdata and native RequestField for explicit field MIME. The HTTP transport remains http.client; only multipart requires the explicit urllib3 dependency. Do not write a wire encoder or silently add a runtime requirement to MoleAPI.
- aiohttp retains ClientSession/FormData and native data/json arguments; add binary files, safe UTF-8 text/control bytes and root JSON null handling. Native context owns its session. Preserve selected Content-Type and generated multipart boundary headers.
- Extend fixtures/verifier to all five Python libraries and13 existing profiles; add text that parses as JSON, mixed newline binary bytes, and explicit multipart field MIME/Unicode filename. Code and tests come first, then batch syntax/native HTTP and upstream-failure contrasts.
- Enable repeated forms and filename-only binary capability only for the two newly implemented libraries. Test default privacy, owner API and immutability. Keep all53 other adapter outputs unchanged.
- UI applies ui-ux-pro-max and emil-design-eng: disclose http.client multipart dependency and aiohttp async/file prerequisites only for the returned snippet; use existing text/theming, natural wrapping and keyboard focus.
- Preserve text-only multipart in aiohttp and Requests/HTTPX native builders as well; repeated text/empty values retain multipart MIME and source order. Manual multipart headers remain rejected by canonical validation.
- Finish with generation/API/UI tests, deterministic engine rebuild, focused lint/build, actual owned-API bilingual/narrow UI, exact evidence/coverage, explicit commit/cherry-pick/push. Preserve pending Apple commits and existing drafts. Full plan remains active.

## Verified checkpoint

80 original-code syntax/native HTTP combinations passed (5 clients×16 profiles), including the39 prior Requests/HTTPX cases. Current tools: Python3.14.8, aiohttp3.14.5, Requests2.34.2, HTTPX0.28.1, urllib3 2.8.0. All10 contrasted previous defects reproduced.95 generation/3 snippet API/41 UI-i18n cases pass, plus scoped strict Clippy, per-package formatting, frontend/server builds and deterministic rebuild. Complete clean upstream patch application matches all Python emitter sources.53 other representative adapter outputs remain byte-identical. Actual owned-API browser checks on en/light1280 and zh/dark375 show the respective http.client/aiohttp guidance and native encoders, privacy, no document overflow, axe0 verified violations/1 manual contrast check. Screenshots inspected. Evidence: `/tmp/moleapi-python-native-final-3` and `/tmp/moleapi-python-native-*.log`.

The initial metadata fixture with an authored boundary was correctly rejected by canonical validation. Retained that rule and verified mature generated boundaries with valid saved requests; did not bypass input validation. Remaining advanced shapes/empty parts/streaming/OS/dependency/method/auth/transport scopes are recorded in SNIPPET-COVERAGE, with the complete plan still active.
