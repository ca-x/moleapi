# Native generated CLI typed pagination and raw errors

Parent: code-generation.md, remaining milestone2.

Retain Progenitor's actual Dropshot pagination contract (items/optional next_page,
optional page_token and positive limit, x-dropshot-pagination first-page required params).
The CLI outputs one JSON item per line. A raw error from a paginated operation must
return the SDK error/nonzero exit rather than emit todo!(). Previous emitted items may
remain on stdout on a later-page failure; no false success/end record is emitted.

Write implementation and standalone fixtures before testing: full multi-page traversal,
Unicode/reserved page-token encoding, first-page-only query removal, Bearer/header fidelity,
limit1 and limit spanning pages with no extra fetch, first/later raw400 failure,
missing required first-page param/zero limit/manual page-token rejection. Add owned-worker
API/privacy and artifact regression coverage. Run source/check/runtime gates, preserve
raw-page/non-Dropshot pagination limits, document exact evidence, commit/merge/push.


Runtime diagnosis also reproduced a fixed NonZeroU32 lookup against a schema-derived
NonZeroU64 parser. Repaired lookup/conversion without changing limit assertions. Explicit
incompatible declared pagination (raw success/missing token/invalid next_page) now rejects
before emission instead of silently degrading to one-shot behavior.

Verified: both bounded integer/explicit uint32 variants passed18 requests; all5 standalone
body/binary/upgrade/pagination fixtures passed38 requests,70 generation regressions and11
owned-worker API cases passed with strict scoped Clippy/formatting. Broader pagination,
raw-success transport and native-platform requirements remain open in parent coverage.
