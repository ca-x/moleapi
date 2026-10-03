# Execution module implementation report

Implemented in `crates/core`, `crates/server`, and the new `crates/script-runtime`. Root owns frontend, formats, workspace manifest, desktop, CI and publication. No commits made by this implementer.

## Concrete behavior

- `Pair.local_value` is optional; workspace, collection and request scripts default to empty strings. `WorkspaceData.global_variables` and `Collection.variables` default to empty arrays. Older JSON documents still deserialize.
- Scope precedence is temporary > execution-data > environment > collection > project. `pm.globals` refers to project variables; there is no fake team-global scope.
- Native stored local values override shared values. Hosted storage scrubs local values on create/update. Native cloud upload sends only scrubbed shared data; cloud comparison/fingerprinting ignores local overrides. Pull restores native project/collection/environment overrides by stable parent ID plus variable ID/key.
- QuickJS uses `rquickjs 0.14.0`, with no filesystem, process, module loader, timers or independent network APIs. Every phase runs in its own worker process with a 64 MiB VM heap cap, 512 KiB VM stack cap and a 250 ms cooperative interrupt. A separate 1 second absolute host deadline covers serialization/startup/IPC and kills/reaps overdue workers; cancellation kills them as well. Native BigInt work cannot rely on VM polling, so the worker watchdog is authoritative. Four script phases can execute concurrently per router; excess phases receive a capacity error rather than allocating unlimited engines.
- Pre order: project, collection, request. Post order: request, collection, project. Scripts within one phase share a JavaScript context; pre and post JavaScript globals do not carry across phases. Use `pm` variable scopes to pass data. Successful scope mutations carry through the network phase and between requests in one runner; workspace snapshots are not silently modified.
- Pre-script errors abort networking and return an API error. Post-script errors preserve the HTTP response and add a failed `Post-response script` test. Runner counts script failures and proceeds to subsequent requests.
- Console, script tests, variable diffs and actual pre-script request field diffs are returned only for the live execution. History excludes these ephemeral outputs and redacts known local/temporary/data/script-generated/auth values from persisted response text, URL and test text. JSON escaping, URL-path escaping, marked query values and bare bearer tokens from explicit Authorization headers are covered. Known-private binary responses are omitted from history. Replacement expansion is bounded with Aho-Corasick; a history string that would exceed 5 MiB becomes a redaction marker.

## API additions

`POST /api/execute` and `POST /api/workspaces/:id/run` accept optional arrays:

```json
{
  "variables": [{"id":"v","key":"temporary","value":"value","enabled":true}],
  "data": [{"id":"d","key":"iteration","value":"value","enabled":true}],
  "locals": [{"scope":"environment","key":"token","value":"browser-private-token"}]
}
```

`variables` is temporary/effective scope; `data` is read-only execution data. `locals` is restricted to project, collection and environment scopes and is applied exclusively to isolated execution state. All overrides are private for history purposes. Native local values are already provided by stored `Pair.local_value`; hosted browsers can retain their own private local store and send `locals` with execution only.

Response additions (default empty arrays):

- `logs: [{level,message}]`
- `variable_updates: [{scope,key,value?}]`
- `request_updates: [{field,value}]`, where field is method, URL, headers, body_kind or body; headers value is JSON text. These describe pre-script changes before interpolation.

Variable update scope names are exactly `project`, `collection`, `environment`, `temporary`. Omitting value means unset. Within a runner unset removes the effective scope key. A browser can apply project/collection/environment updates to its matching local store: value sets a local string; omitted value removes that local override, so a future independent execution falls back to shared data. Temporary updates should remain temporary. Applying these diffs never requires overwriting the shared workspace. Browser stores must key project values by workspace, collection values by workspace/collection, and environment values by workspace/selected environment.

## Supported pm compatibility v1

- `pm.variables`, `pm.environment`, `pm.globals`, `pm.collectionVariables`: get/has/set/unset/clear/toObject/replaceIn. `pm.variables.get` follows full precedence.
- `pm.iterationData`: get/has/toObject/replaceIn; mutation errors explicitly.
- `pm.request.method` and `pm.request.url` string assignment; headers get/has/all/toObject/add/upsert/remove; body raw getter/setter and update(string or {raw,mode}). Existing HTTP body validation remains authoritative.
- Post-response: code/status/responseTime/responseSize, text/json, read-only headers; `pm.response.to.have.status` and `pm.response.to.be.ok`.
- `pm.test` callbacks and `pm.expect`: equality, deep equality, include/contain, regexp match, numeric comparisons/range, lengthOf, a/an, property, true/false/null/undefined/exist/ok, fluent chains and negation.
- Console log/info/warn/error/debug. `pm.compatibilityVersion` is `moleapi-pm/1`.

Bounds include 256 KiB per script, 1000 entries per variable scope, 1 MiB combined effective scope storage, 4 MiB/5000 retained private values, 200 tests/logs and 1000 updates per phase, 64 KiB script log/test text, 16 MiB engine input and 8 MiB engine output. Existing 5 MiB string/20 MiB request expansion limits remain in place. Source code itself is not interpolated.

## Verification

Full implementer checks before the final quota/matcher edge fixes; the authoritative final serial root checks are recorded below:

- `cargo test -p moleapi-core -p moleapi-server -p moleapi-script-runtime`: **58 passed**, including hosted embedded-asset routing checks (19 core, 11 runtime/worker, 28 server).
- `cargo test -p moleapi-core -p moleapi-server -p moleapi-script-runtime --no-default-features`: **57 passed** (native IPC/no embedded web feature).
- `cargo clippy -p moleapi-core -p moleapi-server -p moleapi-script-runtime --all-targets -- -D warnings`: passed.
- Native/no-default clippy, `cargo fmt ... --check`, and `git diff --check`: passed.

Meaningful coverage includes genuine JS object/array processing, request mutation over real networking, pre->network->post composition, all five precedence levels, scoped hosted private values reaching pm and networking, no account leakage, network-policy enforcement, runner chaining/failure counts, infinite loops, catastrophic regexp backtracking, oversized ArrayBuffer/allocation/output, engine isolation, prototype-named initial variables, Unicode output bytes, unsupported APIs, variable update bounds, cloud scrubbing/no-op comparison/pull preservation, escaped private values and bounded history replacement.

## Limits and remaining modules

This is a usable versioned compatibility subset, not full Postman compatibility. `pm.sendRequest`, filesystem/process APIs, imports, timers, async scripts/callbacks, cookie jars, Vault and request-flow controls explicitly error. Implementing a future sendRequest adapter must reuse the existing bounded transport and redirect policy. URL assignment is a string API rather than Postman's full URL SDK. No native OS-Vault integration is claimed. Failed script phases discard partial logs/tests/mutations; post errors still produce a failed result.

The full feature matrix remains broader: team membership and team globals, richer auth/body adapters, other protocols, enterprise features and further module families are separate actual implementations. Low-level `moleapi_core::execute` remains the bounded HTTP transport; the server/IPC execute and runner orchestrate `moleapi_script_runtime` around it to avoid crate dependency cycles.


## Independent review fix round 1

The reviewer reproduced six defects. New tests first failed on all three runtime semantic defects and both history leaks; those regressions now pass. The native BigInt bypass was reproduced independently at 3.37 seconds despite the old 250 ms cooperative limit; it is now contained by process termination instead of a syntax blacklist.

- Query/form privacy: `private value/+?` becomes `private+value%2F%2B%3F`. The matcher includes application/x-www-form-urlencoded and percent-space variants, retaining the original private value even after post-script unset. Router regression confirms both saved URL and query-echo body are redacted.
- Failed-phase privacy: a Rust setter callback records attempted private values before scope mutation. Failure metadata carries taint through the private worker protocol without committing request/variable changes. Generated Date.now values and response-derived values no longer enter saved error/body fields. Saved failed-script details are withheld; killed/crashed/over-budget workers cannot certify taint completeness, so affected history text/headers/URL/test details are withheld conservatively. Failure Debug omits private taint contents.
- Detached async: a QuickJS promise hook rejects promise creation even when a settled/detached async IIFE is followed by a non-promise result. Rejected and resolved detached promises, explicit constructors and callbacks have regression coverage.
- Prototype keys: toObject uses own-entry construction, preserving __proto__ in temporary/project/environment/collection scopes.
- Property negation: one combined presence/value predicate is negated, so {x:1}.not.have.property('x',2) passes while the matching value fails; missing and null-target cases are also covered.
- Hard CPU containment: worker tests kill native BigInt exponent/division at the 1 second host deadline, verify cancellation kill/reap, cap stdout and suppress stderr. Router tests verify a pre-timeout never reaches networking and a post-timeout retains the live HTTP response while withholding uncertain saved data.

Ownership was extended to server and desktop main entrypoints for worker dispatch. Public execute/run JSON remains unchanged. Existing constructors remain compatible; additive local_with_worker/hosted_with_worker accept explicit trusted executable paths for library embeddings. Router tests use CARGO_BIN_EXE_moleapi-server rather than PATH lookup. In-process run remains cooperative and must not be used for untrusted user scripts. Worker protocol bounds are 24 MiB input /32 MiB output including privacy metadata; VM input/output bounds remain 16/8 MiB.

Full implementer suites passed at 58 default /57 native tests before the quota/matcher edge fixes below; both clippy modes passed at that point. Desktop cargo check failed because this host lacks webkit2gtk-4.1/javascriptcoregtk-4.1 pkg-config system packages, so no local desktop compile success is claimed. Independent re-review and root's final serial suites, rebuilt application and CI verification remain release gates. No commits or frontend changes made by this implementer.


## Quota boundary and bounded matching follow-up

Independent re-review found that a script could catch the Rust taint-capacity exception and still reach an ordinary Success result. Any privacy_complete=false state now forces a structured phase failure before results are returned, even when JavaScript catches the original exception. Earlier mutations in that phase are not committed. The exact four-1MiB-value byte-quota worker regression passes; a real router regression confirms live response preservation with failed tests, no variable diffs, and conservative history withholding.

That router regression also exposed an independent CPU issue: compiling four overlapping ~1MiB Aho-Corasick patterns consumed sustained CPU for minutes. The owned test was terminated, and compilation was bounded at the source. Incomplete privacy bypasses pattern compilation entirely because all affected text is already withheld. Complete-privacy matching accepts at most 4KiB per raw private value,16KiB combined raw private values, and64KiB combined encoded patterns; larger inputs cause conservative saved-text withholding instead of expensive compilation. An oversized-overlap regression now returns immediately. Stable history entry/workspace/request identifiers and normal metadata remain usable; response content and request names may be withheld for privacy-limit cases.

Fresh targeted evidence after these final fixes:

- `cargo test -p moleapi-script-runtime --test worker worker_rejects_caught_privacy_capture_overflow`: passed (0.20s).
- `cargo test -p moleapi-server --test scripts caught_privacy_overflow`: passed (0.29s).
- `cargo test -p moleapi-server --lib oversized_overlapping_patterns`: passed (0.00s).
- Formatting and diff whitespace checks passed; no implementer Rust builds/tests remain active. Final Linux owned counts are61 default /60 no-default; root serial verification passed all67 workspace tests (including formats), clippy all targets with -D warnings, rustfmt all and the standalone env-cleared worker smoke.


Final root verification after all quota and bounded-matcher fixes: **67 workspace Rust tests passed**, all-target clippy with `-D warnings` passed, rustfmt all passed, and the standalone env-cleared worker smoke passed. Root also reports26 frontend tests, TypeScript/production build and frontend review passing. Local desktop cargo check remains blocked by missing WebKitGTK4.1/JavaScriptCoreGTK4.1 pkg-config system packages; cross-platform server/desktop and Docker worker smoke checks were added to CI by the root. No implementer builds remain active. Independent budget-fallback acknowledgement and final release/commit/publication remain with the root.
