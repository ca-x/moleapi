# Mock rate-limit implementation

1. [ ] Add backward-compatible `rate_limit_per_minute` and 0–1,000 validation, native backup roundtrip and external export rejection.
2. [ ] Add database-timed sliding-window admission with one transactionally locked workspace row and one owner-scoped bounded counter per saved example. Reset counters in the workspace CAS transaction and remove them on deletion. Cover current DB clock and corruption/cancellation/owner boundaries.
3. [ ] Return fixed 429 plus `Retry-After` before fault/schema/script/delay, and record `rate_limited`. Prove direct/routed/public/native real wire, separate owners, cross-instance concurrent admissions and revision/revocation behavior.
4. [ ] Add bilingual editor input and distinct call-log label, with numeric bounds, preview invalidation, save/reload and 390px browser call.
5. [ ] Run full applicable suites, strict Clippy/format/typecheck/build, independent review and fixed-head three-database/Docker/native CI. Record exact evidence and remaining Mock module scope.
