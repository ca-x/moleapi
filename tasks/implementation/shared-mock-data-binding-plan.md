# Shared Mock dataset bindings

Authority: `docs/specs/workspace-collaboration.md`, shared member editor parity.

1. Extract the existing owned `MockDataEditor` as a pure controlled component. Preserve dataset/view/shape and ordered bounded request-parameter editing.
2. Add v2-only new data-source selection in shared Mock source controls using the canonical draft datasets; keep existing bindings selected when missing or v1. Clear Schema/link/inference only on explicit source change and retain body/script/traffic/matching. Normalize enabled Content-Type to JSON like the owned editor.
3. Render the extracted binding editor in the shared view. Parameterized bindings require the member routed endpoint; disable direct preview and show its existing translated hint. Do not call owner-only dataset resource routes.
4. Tests first fail for absent source/binding controls and preview gate. Cover CAS preservation, shape/parameter changes, stale dataset/view alert, v1 selection gate. Add real shared routed dataset parameter coverage using the canonical owner dataset.
5. Run affected server and full frontend tests, production build then strict server Clippy and embedded build sequentially, format/diff checks, independent source review and real 390 px bilingual member editing/routed response verification. Record evidence and push to PR41.

Remaining: dataset management UI, specification-link selection and other collaboration requirements remain separate; this slice does not complete the overall goal.

## Execution record

- RED: new shared tests failed because dataset selection and parameter controls were absent.
- Extracted the existing owned binding editor without changing its selection/parameter behavior; shared controls use only canonical draft datasets and shared workspace CAS.
- Independent source review found one Important issue: v2 non-HTTP requests advertised a binding rejected by core validation. A new WebSocket regression failed first, then data-source eligibility was restricted to HTTP plus v2. No second concrete issue was reported.
- GREEN: full frontend 507 tests, focused owned/shared controls and canonical owner routed dataset test. The latter saves through member CAS, queries two real owner rows using the incoming path parameter and denies viewer/bare-owner calls.
- Real embedded-browser Save exposed omitted empty parameters from core serde serialization, causing owned/shared `.length` access to fail. New owned initial-load and shared canonical-Save regressions both failed first. The API type now marks parameters optional; the extracted editor treats omission as an empty array and both preview gates use optional access.
- Verification: full frontend 507 tests passed; routed integration 3 passed. Production build verified 45 chunks, strict server Clippy, package formatting, diff check and embedded server rebuild passed.
- Runtime: Chinese member UI loaded the normalized omitted-parameter binding and directly previewed both rows. It then selected By ID, first row and a path ID parameter, saved, and member-token requests for IDs 7 and 8 returned the corresponding owner row. The original static body persisted and direct preview was disabled. English member UI changed shape to all matching rows, saved and returned the corresponding array. Both 390 px views fit; a fresh browser session had no errors. Captures are linked in the collaboration coverage document.
- Ruling: normalize the optional empty-parameter wire field in the shared pure editor and both owned/shared preview gates, rather than alter core serialization; this matches the existing canonical API and repairs the owned initial-load case too. Cost if wrong: optional parameter handling would need a boundary migration.
- Final review: the one Important source-review finding is covered by a failing-then-passing WebSocket regression; the independent reviewer found no second issue. The later runtime default omission is covered by failing-then-passing owned/shared regressions and the fresh rebuilt browser acceptance.
