# Independent request snippets review

2026-10-04; application worktree; uncommitted slice atop3d8594d. Authority: docs/specs/code-generation.md and docs/superpowers/plans/2026-10-04-request-snippets.md. Read-only source review with /tmp-only independent Rust probes; only this evidence report written. Ignore old scratch tasks/diffs. Root is fixing the findings; final post-fix verdict follows below when verified.

## Concrete pre-fix findings

1. **High — default snippets leak literal sensitive query values when the URL contains environment placeholders.** In the reviewed original `crates/formats/src/redact.rs` generation_request URL fallback (then line738; now fallback is corrected at line754), Url::parse failure falls back to raw screen_text. ExportPrivacy::from_workspace originally collected URL credential values only on successful Url::parse. A valid saved draft `{{base_url}}/echo?token=private-query-seed` therefore emits the literal secret and its nonsensitive JSON body copy with include_secrets=false. Exact /tmp/snippets-review-probe.rs printed `saved validation: Ok(())` followed by Node fetch containing both query token and JSON copy `private-query-seed`. No workspace guard prevented this input: template-bearing URLs are deliberately permitted. Root notified; new code separately parses query pairs using mature url::form_urlencoded and screens the fallback.

2. **High — combined Basic credentials copied as Base64 bypass default privacy.** Original ExportPrivacy::from_workspace collected username/password separately but did not collect the combined username:password before computing encoded patterns. Valid Basic auth loginabcde:super+secret has Base64 `bG9naW5hYmNkZTpzdXBlcitzZWNyZXQ=`; neither standalone Base64 pattern matches this value. A nonsensitive JSON body copy survives default generation. Exact /tmp probe printed `BASIC saved validation: Ok(())` and generated Node JSON containing that Base64 credential while include_secrets=false. New collection at `crates/formats/src/redact.rs:477` joins Basic credentials before existing encoding expansion. Root notified and added a focused valid-workspace regression.

3. **Medium — repeated urlencoded form fields silently collapse in multiple advertised adapters.** Original HAR mapping passed params for every form request without checking target representability (`crates/generation/src/request.rs`, original line86). Exact /tmp/snippets-shapes-probe.rs, with validate_workspace Ok(()), generated Node fetch from x=one&x=two as `new URLSearchParams({x:'two'})`; Go emits Values.Set twice and likewise sends only the final value. Shell/cURL emits both data-urlencode rows and preserves both. Existing test asserted only curl preservation. New adapter gate at current `crates/generation/src/request.rs:82` rejects repeated form fields outside proven shell/curl. This explicit limitation avoids silent request corruption without introducing handwritten emitters.

4. **Medium — configured auth fails to replace manual Authorization headers in generated snippets.** Original request adapter retained all enabled headers and appended auth-derived Authorization (`crates/generation/src/request.rs`, original line60). A valid manual Authorization=Bearer manual-auth plus bearer auth token computed-auth yields two curl Authorization headers, while shared core request_headers inserts configured auth and has only one. Exact shape probe printed `auth saved validation Ok(())`, `transport Authorization count1`, and curl with both manual and computed headers. Upstream servers may select the first header or reject duplicates; resulting behavior differs from MoleAPI execution. New filter at current `crates/generation/src/request.rs:72` drops manual Authorization when configured auth is active; shared framing/hop header removal is also matched.

## Independent evidence before fixes

- `cargo test -p moleapi-generation --locked`: exit0,4 actual tests passed, including every upstream plugin smoke.
- `cargo test -p moleapi-server --test generation --no-default-features --locked`: exit0,2 actual owner/privacy/native-route tests passed.
- `npm --prefix web test -- --run src/features/generation/GenerationDialog.test.tsx`: exit0,2 async generation/UI tests passed.
- /tmp Rust probes linked real freshly built generation/core/serde artifacts, did not edit workspace sources. Outputs show above failures on valid-workspace inputs; exact transcripts retained in reviewer turn.
- Encoded URI userinfo probe separately returned validate_workspace Err(URL credentials are not supported); raw generate_request can expose decoded copies there, but service accepts only saved validated workspaces. This unreachable service scenario is excluded from findings.

## Reviewed boundaries and evidence limits

Read generation runtime/request adapter, formats privacy, owner-scoped server routes/slots/tests, generation dialog identity/copy/download guards and editor integration, pinned build entry/lock/bundle/manifest/licenses, compile-time SHA/version gate and trusted fixture verifier source. QuickJS gets no host network/filesystem/process callbacks; new runtime per call has memory/stack/time/input/output bounds; four service slots remain held by actual blocking worker. Invalid engine target errors omit source values. Default generation preserves source templates and does not resolve local variables or execute pre-scripts; explicit saved credential inclusion is visible. UI hides late results on workspace/account/request/target/client/credential changes; source screenshot/browser/native graphics gates remain root-owned.

Catalog availability is explicitly engine-smoke evidence; no claim of TypeScript/C++ emitters or typed SDK/server completion. Root reported seven trusted generated fixture programs compiled/executed; that is reused root evidence, not an independent reviewer run. This review does not treat catalog listing as language validation. No new broad audit or protected fixture/process changes.

## Post-fix recheck of first four findings

Fresh `cargo test -p moleapi-generation --locked` exited0 with7 nonempty tests; generation service2 tests and UI2 tests also exited0. Recompiled exact original /tmp probe inputs against rebuilt artifacts. Templated sensitive query becomes a redacted placeholder and copied JSON becomes [REDACTED]; combined Basic Base64 copy becomes [REDACTED]; Node/Go repeated forms return explicit adapter errors; curl preserves both fields; configured auth leaves only one generated Authorization matching shared transport. Source gate also rejects unverified repeated headers and fetch-family GET/HEAD bodies, and filters transport-owned framing/hop headers. Those four findings resolved.

## Fifth concrete finding — structural privacy keys

**Medium — common credential values make default generation fail.** `crates/formats/src/redact.rs:808` (reviewed pre-fix source) passes the full typed RequestSpec DTO through ExportPrivacy.screen_json, whose object visitor removes every key containing any known private pattern. Valid Basic username=user matches fixed key auth.username, so that required structural field is removed before deserialization and generation fails with Request cannot be safely screened for generation. Similar common values token/password collide with structural field names. Exact /tmp/snippets-structural-probe.rs independently printed `BASIC saved validation: Ok(())` then `BASIC Err(Request cannot be safely screened for generation)`. Root notified. Preserve fixed DTO field keys while screening their values; actual copied user-defined JSON keys remain subject to privacy screening. Fifth finding awaits fix/recheck; no new broad audit needed.

## Fifth fix and final scoped recheck

Fifth structural-key finding resolved. Generation privacy now screens transported user fields and JSON body user keys, preserving fixed RequestSpec/Pair/Auth field names. Recompiled /tmp/snippets-structural-probe.rs against latest built libraries: valid Basic username=user now generates successfully. Re-ran exact original first-four inputs: sensitive templated query/body copies still redacted, exact configured Basic tuple Base64 still redacted, Node/Go repeated forms explicitly rejected with curl preservation, configured auth emits one Authorization.

Fresh final scoped gates all exited0:
- `cargo test -p moleapi-generation --locked`: 8 passed (including every upstream plugin, privacy, structural keys, representability and auth/header regressions).
- `cargo test -p moleapi-server --test generation --no-default-features --locked`: 2 passed after latest structural fix.
- `npm --prefix web test -- --run src/features/generation/GenerationDialog.test.tsx`: 2 passed after latest dialog width change.
- `git diff --check`: exit0.

All five concrete findings independently resolved. Scope review otherwise found no material runtime/ownership/UI race/bundle-provenance issue. Root owns full source/frontend/actual browser/native/distribution gates and seven representative target fixture evidence. Root narrowed dialog width after browser observation; source inspected, visual outcome remains root browser evidence.

Advisory copied-encoding boundary sent to root: patterns mask direct known secret encodings and the exact current Basic username:password tuple. The structural probe intentionally changes configured username but retains a body Base64 string of the previous username with the same known password; that old tuple survives. A bounded decode-and-screen of user Base64 values would also detect arbitrary prefixed/old-username copies. This is beyond the five confirmed fixes and needs a stated privacy boundary or follow-up, not a claim that every transformed encoding is screened. Encoded URL userinfo remains invalid in saved-workspace service inputs.

No source changes by reviewer. Review snapshot/evidence should remain task-local; canonical scope and capability status belong to maintained docs.

## Final advisory closure and verdict

Status: scoped request-snippet implementation approved; all five concrete findings resolved. Adjacent old-username encoded-copy advisory also addressed. New generation-only text screening decodes complete Standard/URL-safe-no-pad Base64 strings or explicit Basic header values up to16KiB using the mature base64 crate; it screens decoded UTF8 against known private values and withholds the original value on a match. No decoded data enters output. JSON body user keys/strings and parsed original/fallback URL query, form fields and Pair key/value use this pass; fixed DTO keys remain intact. Arbitrary transformations, mixed opaque blobs and larger encoded fields remain explicit detection limits.

Fresh latest `cargo test -p moleapi-generation --locked`: exit0,8 passed including expanded old-tuple JSON/URL/form regression. /tmp/snippets-encoded-fields-probe.rs independently reran valid Basic username=user with old loginabcde:super+secret Base64 copy: JSON output [REDACTED], original URL query copy=%5BREDACTED%5D, and placeholder-base URL/form copy [REDACTED]. Probe exited0 and generated successfully. This reverses the structural probe's remaining encoded-copy observation.

Final review evidence includes8 current engine tests,2 service tests after structural fix,2 UI tests after width fix, exact original valid-input probes plus expanded adjacent copied-value probe, and source/provenance/race/budget review. Root owns final full workspace/browser/native gates and broader required TS/C++/SDK/server code generation. Approval applies to the explicit request-example slice and its documented representability/privacy boundaries, not the full code-generation capability module.

## Sixth browser finding and final UI recheck

Root browser QA found **medium — first generation can be discarded after an equivalent save response reorders RequestSpec keys**. Original `web/src/features/generation/GenerationDialog.tsx:16` used insertion-order-sensitive JSON.stringify for the request scope identity. save(true) can replace the request object with identical content in a different key order; the post-save current guard then rejects the unchanged request and never calls generation. Root added a regression that reorders keys during save and reported pre-fix failure.

Reviewed fix imports pinned mature fast-json-stable-stringify2.1.0 and builds identity canonically; owner/request/content/target/library/credential/open fences are retained. Fresh `npm --prefix web test -- --run src/features/generation/GenerationDialog.test.tsx` exited0,3 passed including equivalent-key save ordering, real content export and late owner result guards. `npm --prefix web ls fast-json-stable-stringify --depth=0` confirms installed2.1.0; package/lock agree with integrity/version and MIT license; dependency directly used. `git diff --check` exited0. No source edits by reviewer.

Final verdict remains scoped approval, now with all six concrete findings resolved and adjacent encoded-copy advisory closed. Browser width/download and full-source/native gates are root evidence; this review is complete for the explicit snippet slice, without implying broader code-generation capability completion.

## Seventh representability correction

Root found medium upstream Python/http.client behavior: an unresolved base template reaches Scalar python/python3 parseRequestUrl, which falls back to new URL(raw,https://example.com) and invents a concrete host. Reviewed upstream source and adapter fix at `crates/generation/src/request.rs:52`: that plugin now requires a parseable absolute HTTP(S) URL with host before emitter dispatch, with an explicit replacement instruction; Node/cURL placeholder behavior remains available.

Fresh `cargo test -p moleapi-generation parsed_url_adapters_do_not_invent_a_host_for_base_templates --locked` exited0,1 nonempty regression passed (8 filtered). Reviewed assertions reject templated Python base and preserve it in Node output. `git diff --check` exited0. Scoped approval includes this seventh correction; all seven concrete findings are resolved, plus the adjacent bounded encoded-copy pass. Root's full engine9/workspace204/browser evidence remains separately owned and is not presented as this reviewer run.
