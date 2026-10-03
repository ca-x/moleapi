# Protocol fix reviews

**Current status after round3:** all findings from this scoped backend review are resolved. The earlier sections retain the historical findings and evidence; the final round3 section supersedes their open statuses.

Baseline remains `11250e1f4487a33a9ffd5c271e7e3d8878f240bc`, with uncommitted fixes. Scope: the three findings in `protocol-review.md`, new owner admission gates, credential capture/interpolation/worker taint, and the retained-None-body correction. Read actual implementations and added regression fixtures; no source changes, subagents, commits or full test-suite runs.

Verdict: the original literal-header, secret-query and logout repros are fixed. **One P2 residual remains in credential capture for templated header names.** No additional finding in admission fencing or retained body handling.

## P2 — Resolve sensitive header names before capturing original credentials

Source: `crates/server/src/privacy.rs:152` classifies original headers using only `h.key.to_ascii_lowercase()` at line156. Header-name interpolation is explicitly supported by `crates/core/src/validation.rs` and the existing request resolver. Although `request_values` resolves values, it does not resolve names before deciding which values to capture.

Independent trigger:

- Public workspace variable `header_name=Authorization`.
- Enabled request header `{ "key":"{{header_name}}", "value":"Bearer templated-key-private-credential" }`.
- Pre script `console.log(pm.request.headers.get('{{header_name}}')); pm.request.headers.remove('{{header_name}}');`.

POST sessions succeeds and its system `script_log` contains the unmasked `Bearer templated-key-private-credential`. Before the script, the raw template name is not recognized as sensitive; after the script, the header is absent, so the later resolved capture cannot recover it. This is the same owner-bound metadata leak as the initial finding, on a supported interpolation path.

Classify both raw and currently resolved header names before running scripts, retain original credential values across failure/removal, and apply the same rule to extracting the credential suffix from Authorization. Add a regression for a templated key with a literal credential removed before script export. Script-added templated sensitive names should receive corresponding coverage, because `crates/script-runtime/src/pm.js` currently uses the raw added key for immediate taint classification too.

## Fixed original reproduction paths

Executed the original `/tmp/moleapi-protocol-independent-review/repro.py` against the rebuilt worktree server, on an independent ephemeral hosted port and SSE fixture with a fresh temporary SQLite database. Output:

```text
FAILURE_REDACTION 400 {"error": "JavaScript error: [REDACTED]"}
REMOVED_HEADER_LOG ... "message": "[REDACTED]"
QUERY_SECRET_SUMMARY 200 http://127.0.0.1:46075/events?opaque=[REDACTED]
LOGOUT_RACE 200 401 None
```

Then executed `/tmp/moleapi-protocol-independent-review/repro-fix-round.py`, which adds the templated-name case to the same harness. It independently confirms the remaining behavior:

```text
TEMPLATED_SENSITIVE_KEY_LOG ... "message": "Bearer templated-key-private-credential"
```

Both scripts exited0; each terminated its own server/fixture and cleaned its database. All credential values are synthetic. The controller's servers, browser session and database were untouched.

## Fix-path inspection

- Original raw and currently resolvable credential values enter `scopes.private_values` before pre scripts; failed-worker private values are unioned into the retained set, with a fail-closed fallback when redaction cannot be built. Script-added literal sensitive headers taint their values before export or failure. Explicit secret query values participate in the same redactor and its URL/JSON/Base64 variants.
- Admission holds an owner gate through generation/token recheck, registration, final workspace/request ownership check and start. Logout revokes the token, advances the same generation and closes registered owner sessions while holding that gate. Preparations retain the Arc across worker execution, so weak-map pruning cannot replace their live generation fence. New owner gates are bounded; unrelated owners use independent gates. Hosted token validity is checked at both boundaries; native relies on the generation fence.
- Retained body text remains visible to pre scripts and saved request state. Only the execution clone is cleared for resolved body mode None, after request updates are formed and before body interpolation. GET/None validation occurs after scripts, with an additional engine check. Both SSE and WebSocket construction use GET without attaching body bytes. Read the new fixtures asserting saved draft preservation, unresolved ignored templates, empty actual request bodies, and rejection of post-script POST/JSON mutations.

## Verification limits and completion

The worker's75-test broad gate, Clippy and targeted7-API/direct-engine fixture runs were read as reported evidence, not rerun or claimed as independent execution. This recheck independently executes only the hosted privacy/logout harnesses; native fencing and SSE/WebSocket body behavior were reviewed against source and the meaningful assertions in their fixtures. Packaging, frontend, real TLS, and external databases remain outside this scoped recheck.

Completion: original findings rechecked; residual templated-name defect reproduced and reported; fix-path inspection complete; additional fix and targeted regression verification pending. No source changes or public actions.

## Round2 recheck — original templated-name case fixed; alias-change failure remains

Re-read latest `privacy::request_values`, the pm runtime's `captureHeader`, expanded regressions, and the implementation report. The original templated header-name defect is fixed: the exact independent `repro-fix-round.py` now shows `[REDACTED]` for the templated header log, along with the original masked400/log/query outputs and logout200/create401. Raw/resolved header and query name classification retains explicit secret flags. No unrelated new issue was found in this scoped pass.

**P2 residual:** an existing header can become sensitive through a variable update and fail before the next header API operation. In `crates/script-runtime/src/pm.js`, `captureHeader` runs on add/read/remove, while `store.set` and `store.unset` change effective name resolution without reclassifying existing headers. `prepare_live` receives no final request on script failure, so its later Rust credential scan cannot recover the newly sensitive value.

Independent minimal script (start with no request headers):

```javascript
pm.variables.set('header_name', 'X-Public');
pm.request.headers.add({key: '{{header_name}}', value: 'Bearer alias-before-throw-credential'});
pm.variables.set('header_name', 'Authorization');
throw new Error('alias-before-throw-credential');
```

Observed from `/tmp/moleapi-protocol-independent-review/repro-alias-failure.py`:

```text
ALIAS_CHANGED_THEN_THROW 400 {"error": "JavaScript error: alias-before-throw-credential"}
```

The first add correctly sees a nonsensitive header. The second scope update makes the effective name Authorization, but there is no read/remove before failure to trigger capture. The existing alias-after-add regression removes the header before returning and therefore exercises a different path. Reevaluate sensitive mutable headers when scope updates change effective resolution (including unset/clear fallback), or ensure failure collection captures the current sensitive values before returning details; preserve fail-closed handling if such collection fails. A directly thrown synthetic value demonstrates the gap, without claiming arbitrary JavaScript dataflow tracking or cross-account disclosure.

Verification: both independent Python harnesses exited0 against the rebuilt server on their own temporary database and ephemeral ports. No Rust suite, frontend build, source edit or controller service was touched. The worker's50-test/lint results were read but not independently rerun. Status after round2: original three findings and original templated-key reproduction fixed; one adjacent templated-name failure-path P2 remains.


## Round3 final scoped recheck — remaining finding resolved

**Verdict: no open findings in this scoped backend review.** The last alias-change/direct-throw reproduction now returns masked metadata. Earlier direct-header, removed-header, explicit-query-secret, templated-header and logout reproductions remain fixed.

Independently executed `python /tmp/moleapi-protocol-independent-review/repro-alias-failure.py` against the current rebuilt worktree server. It exited0 on its own temporary database and ephemeral hosted/SSE fixture ports, then cleaned those resources. Observed:

```text
FAILURE_REDACTION 400 {"error": "JavaScript error: [REDACTED]"}
REMOVED_HEADER_LOG ... "message": "[REDACTED]"
QUERY_SECRET_SUMMARY 200 http://127.0.0.1:40621/events?opaque=[REDACTED]
TEMPLATED_SENSITIVE_KEY_LOG ... "message": "[REDACTED]"
ALIAS_CHANGED_THEN_THROW 400 {"error": "JavaScript error: [REDACTED]"}
LOGOUT_RACE 200 401 None
```

Re-read the final fix and adjacent callers. Scope set/unset captures the current request before and after mutation; clear delegates to unset, preserving values exposed by fallback scopes and values that become nonsensitive later. Current capture includes auth, headers, saved query rows and URL metadata. URL callbacks use mature Rust URL/percent-decoding support. Unknown aliases conservatively capture raw candidates and mark failed metadata uncertain while allowing legitimate successful pre-fill. Rust-owned capture transaction counts and persistent failure flags prevent caught/aborted capture work from later claiming complete privacy metadata. Capture remains subject to explicit work/value/field bounds and the existing isolated worker limits. No new concrete correctness or integration finding was identified in this change.

Read the expanded API regressions for direct throw, set/unset/clear fallback, URL mutation, query aliases, initial missing-alias pre-fill and ordinary public metadata remaining visible. Read the runtime regression for caught quota/cycle failures remaining incomplete. These are meaningful assertions, not independent test executions by this reviewer. The implementer's52-test runtime/server, strict Clippy and format checks were not repeated. This reviewer additionally ran `git diff --check`, which passed.

Completion: last open finding independently verified fixed; related original reproductions remain fixed; new capture path inspected; no source edits, full-suite reruns, browser/controller-service use, commits or public actions. This is a scoped backend fix verdict, not a packaged desktop, external database, TLS or frontend verification claim.
