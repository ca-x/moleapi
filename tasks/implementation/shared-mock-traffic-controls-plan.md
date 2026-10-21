# Shared Mock traffic controls

Authority: `docs/specs/workspace-collaboration.md`, role-aware shared Mock editor parity.

- Extract the existing owned delay/fault/rate inputs into a pure `MockTrafficControls` component receiving an example and a partial-example callback. It must not depend on owned workspace context or call owner-only APIs.
- Reuse it in both owned and shared editors. Keep the existing ranges and defaults: delay 0..30000 ms, fault rate 0..100%, fault code 500..599 (default 503), rate limit 0..1000 per sliding minute. Nonfinite inputs do not update the example; finite values use the same integer rounding and clamping as the owned controls.
- Shared changes remain a draft until the canonical owner/workspace CAS Save. Preserve schema/script/data/matching fields when changing traffic settings. Viewers receive no write controls, and dirty/conflicting drafts cannot feed saved-example preview.
- First add a failing shared-view regression for saving fault/rate settings while preserving unrelated fields. Existing owned input boundary regressions must remain green after extraction. Verify full frontend, type/build and actual embedded English/Chinese 390 px save/503 fault/429 rate-limit journeys, then update coverage and push the draft PR. Broader schema/script/data/matching editors and remote/native gates remain open.

## Execution record

- Shared traffic-save test first failed because the fault field was absent. After extraction, both shared and owned controls passed their tests. The catalog checker initially rejected bare translation-key literals; render-time literal `t(...)` calls fixed it.
- Actual disable-fault acceptance exposed a false dirty draft after server default omission. The new regression failed before the snapshot-aware Save acknowledgement and passed after it; continued edits during a pending Save remain protected.
- Full frontend 491/491, owned/shared focused 59/59, TypeScript, production build with 45 chunk checks and rebuilt embedded server passed. Independent review found no concrete regression. Actual bilingual 390 px browser save/fault503/rate201-to-429/disable-to-201 and canonical owner preservation checks passed. Remote CI/native acceptance remains pending.
