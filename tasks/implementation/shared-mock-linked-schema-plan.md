# Shared Mock linked Schema selection

Authority: `docs/specs/workspace-collaboration.md`, member HTTP Mock editor parity and owner-scoped resource access.

1. Add failing integration coverage for `POST /api/owners/{owner}/workspaces/{workspace}/mock-schema/validate`: canonical owner HTTP/spec/method checks, explicit revision, supported response metadata only, viewer/foreign/bare-owner denial, unsupported Schema and actor audit without source/pointer content.
2. Add a member wrapper around the existing bounded owner validator. Lock canonical workspace for role/revision admission and actor `started` audit, reuse the parser/semaphore, watch membership/revision while waiting, and make final access/revision checks the last await. Keep owner-only route intact; classify the new route exactly once in route inventory.
3. Extend the shared source controls to enumerate the request's linked OpenAPI JSON responses via existing `openapiMockChoices`. Preserve stale links with a clear alert and explicit static/inline choices. Parent validates through member route before applying status/media header/link, retains static body/script/traffic/matching, clears incompatible data/inline sources only on success.
4. First failing client tests cover missing selection/route, CAS output, dirty-save gate, validation failure/source retention and delayed result discard after another edit/selection/role/account/revision. Validation state must not overwrite edits made during an awaited request.
5. Run affected server tests and full frontend, production build then strict Clippy then embedded server rebuild sequentially, package formatting and diff checks. Independent review plus real Chinese/English 390 px member select/save/generated-preview acceptance precede commit/push and PR41 update.

Review focus: delayed validation versus newer source choice; removed/re-added membership; method/spec mismatch; unsupported/stale response schema; dirty unsaved selection versus canonical revision. Dataset management, other protocols and full collaboration release gates remain separate required work.

## Execution record

- RED: shared endpoint absent (viewer request returned 404 instead of policy 403); UI response choices/validation absent.
- GREEN: explicit shared validator wraps canonical owner parsing with actor audit and membership/revision fences. Related server collaboration 13, shared Mock 4, owned Mock 31 pass. Full frontend 517 tests pass, including success CAS, unsupported-source retention, dirty gate, stale link and seven delayed-selection boundaries.
- Independent source review found no concrete privacy/admission/race/source-preservation issue; reviewer did not rerun tests.
- Production frontend verifies 45 chunks; strict server Clippy and package rustfmt pass. The embedded server rebuild passes.
- Real Chromium member pages at 390 px selected/saved linked 201 application/json (id 7) in English and 202 application/problem+json (id 8) in Chinese, preserving the static body. Actual member response headers/status and canonical owner link/header/body were checked. Unsupported 422 Schema failed without replacing the saved 202 source. Actor-only admission audit contains no source/pointer; both languages fit and browser errors are empty. Screenshots are linked in `docs/COLLABORATION-COVERAGE.md`.
- No plan deviations or deferred review findings. Full collaboration/resource/native release gates remain open.
