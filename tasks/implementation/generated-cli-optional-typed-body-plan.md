# Native optional typed request bodies

Parent: code-generation.md and milestone2.

Carry requestBody.required through typed JSON/form SDK and CLI emission as well as raw
bodies. Optional positional bodies use outer Option; optional builder bodies default None,
whole-body setters/body_map explicitly set Some, finalization validates provided bodies.
Apply upstream reqwest json/form encoding only for Some. Retain JSON null as distinct
from omission for nullable schemas. CLI optional bodies do not force property flags or
JSON files, but any supplied partial/invalid body still validates before request execution.

Finish implementation/consumers first, then verify standalone CLI and positional SDK
omission/provided/empty/null JSON, form Unicode, required/partial/invalid rejection and
existing raw/pagination/upgrade/large upload regressions. Test owned-worker privacy APIs,
run generation/lint/format gates, document exact shape coverage, commit/merge/push.


Verified: CLI/positional SDK16 POST requests passed with no extra calls for invalid/partial
bodies. All7 standalone fixture builds/runs passed55 requests,72 generation regressions
and11 owned API cases passed. Strict scoped Clippy/formatting passed. Broad nested/union/
form variants, file/field precedence and native-platform evidence remain open.
