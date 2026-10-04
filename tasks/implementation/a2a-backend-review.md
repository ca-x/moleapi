# Independent A2A backend and supplemental UI/formats review

2026-10-04; worktree application; base b0f41c8; frozen implementation report reviewed. No source edits, commits, protected process changes or external actions. Root requested supplemental UI/formats review after separate reviewer allocation failed. Authority: docs/specs/a2a-client.md, docs/superpowers/plans/2026-10-04-a2a-client.md and backend brief.

Status: changes requested; five concrete findings, plus one scoped Stop race. Root notified of all material findings.

1. **Medium — successful HTTP+JSON push deletion reports failure.** `vendor/a2a-client/src/client.rs:1401` routes DELETE through `send_json_request`, which requires application/json at line826 and parses JSON at line834. A server returning successful 204 with empty body (also acknowledged by comment line1400) yields a local serialization error after remote deletion succeeds. Protocol dispatch reaches this path for current HTTP+JSON. Existing push CRUD integration test selects only JSONRPC (`crates/server/tests/a2a.rs:624`). Independent actual-SDK probe `/tmp/a2a-independent-probe.rs` produced: `HTTPJSON empty successful DELETE: Err(SerializationError { message: "Expected Content-Type application/json for DeleteTaskPushNotificationConfig, got '': " })`. Accept successful empty responses in a bounded DELETE path, preserve upstream failure handling, and cover HTTP+JSON CRUD.

2. **Medium — direct HTTP+JSON SSE misses pre-typed JSON work validation.** `vendor/a2a-client/src/client.rs:332` deserializes direct SSE without calling `validate_json_budget`, unlike JSONRPC SSE at line306. The frame/wire caps bound bytes, but >10000 nodes/depth32 in unknown fields are consumed by pbjson ignore_unknown_fields then discarded; outer protocol validation sees only the small decoded object. Independent actual-SDK probe sent a ~50KB direct event with 10001 nulls in an unknown field and a valid message: SDK returned `Some(Ok(StreamResponse ... Message ...))`. Apply the same ingress JSON budget before direct typed decode and cover unknown-field nodes/depth in direct SSE.

3. **Medium — explicitly adopting common advertised versions writes an unsupported dialect.** `web/src/features/a2a/A2aWorkbench.tsx:61` casts interface version directly to A2aDialect then stores it. Backend describes `0.3.0` and `1.0.0` as supported; the legacy fixture uses 0.3.0. Core validation (`crates/core/src/a2a.rs:30`) permits config dialect only 0.3 or 1.0. Clicking Use this interface therefore creates invalid config and blocks saves/connect. Normalize advertised semantic versions into supported config dialect while retaining original card version/source.

4. **High — canonical card default exports leak known private copies.** `crates/formats/src/redact.rs:185` stores canonical Specification.source after only sensitive-field-name redaction. A2A config.card_source receives ExportPrivacy.screen_json but its canonical Specification copy never does. Import/native saves can legitimately contain exact originals and secret-scoped values; a source metadata key/value/numeric scalar containing a known secret under nonsensitive names leaks through default export. Existing card import candidate screening is irrelevant to native imported/saved canonical specifications; include-secrets restore must preserve them. Independent `/tmp/a2a-export-probe.rs`, using the newly built formats library and a global secret `777001` copied in canonical Agent Card metadata key, number and string, returned `canonical A2A default export leaked copied secret: true`. Screen canonical A2A source with the same ExportPrivacy JSON path and test both safe default and exact explicit exports.

5. **Medium — new ownership UI regression test hangs in returned hook cleanup.** `web/src/features/a2a/useA2aCard.test.tsx:11` returns api.mockReset() (the mock function) from beforeEach. Vitest treats the returned function as cleanup; the ownership test leaves api mocked to unresolved Promise, so cleanup hangs. Fresh command `npm --prefix web test -- --run src/features/a2a/model.test.ts src/features/a2a/useA2aCard.test.tsx` exited1, three passed, one failed with Hook timed out in10000ms at line11. Use a block hook with no returned function.

6. **Medium — local discovery Stop does not invalidate a queued success.** `web/src/features/a2a/A2aCardDialog.tsx:17` only posts server cancellation; it does not increment the local epoch or clear pending. If a fetch has completed and its response is queued while the user clicks Stop, validate's current guard at line31 remains true and line36 still attaches the card/closes the dialog. Server final fences cannot retract an already-completed response. Invalidate local generation on explicit Stop, and retain request identity for server cancellation; test a late successful response after Stop. Source-based finding, not browser reproduction.

Independent verification:
- `cargo test -p moleapi-server --test a2a --no-default-features --locked`: exit0,8 passed. All test bodies ran. Legacy vendored dead-code warning observed, not a failure.
- Targeted UI command above: exit1,3 passed/1 hook timeout.
- /tmp-only standalone probes linked existing actual SDK/formats rlibs; compilation initially needed matching duplicate Serde/Tokio artifacts, then all probes ran successfully. No project source modified. The exact commands/results are in review transcript.

Reviewed backend scope: actual injected clients and explicit endpoint isolation; legacy/current dispatch and schemas; shared headers/auth/query/TLS; card canonical source and selected spec attachment; body/SSE bounds and frame decoding; method/parts/history/page bounds; implicit callback rejection and explicit callback checks; owner/logout/deletion/session leases; cancellation workers; event key/scalar privacy and error boundedness. Supplemental scope: A2A UI card import/restore/adoption/invocation/results, shared event mapping/content rendering, A2A export arm and interchange test. Safe media uses existing text/JSON viewers with no automatic URL fetch. No unsupported gRPC claim observed.

Limits: did not rerun entire workspace/Clippy/browser/native graphical QA; root owns those gates. Current protected fixtures/root API were untouched. Source findings require root fixes and refreshed independent recheck before approval.

## Post-fix independent recheck — source/artifacts rebuilt 2026-10-04

Status: all six scoped findings resolved; backend and supplemental UI/formats review approved for these changes. Root owns remaining full workspace/browser/native/release gates.

Fresh commands (all exited0):
- `cargo test -p moleapi-server --test a2a --no-default-features --locked`: 9 passed. New unknown-field direct SSE regression exercises pre-typed structure limits. Push CRUD now loops both JSONRPC and HTTP+JSON against the actual mature fixture.
- `cargo test -p moleapi-formats --test interchange a2a --locked`: 2 passed, including canonical source copied-secret key/scalar/string exclusion and exact include-secrets restore.
- `npm --prefix web test -- --run src/features/a2a/model.test.ts src/features/a2a/useA2aCard.test.tsx src/features/a2a/A2aCardDialog.test.tsx`: 6 passed across3 files. Patch-version normalization, late discovery success after Stop, and ownership guard body all exercised; cleanup no longer hangs.

Recompiled/relinked the exact /tmp-only prior SDK/export probes using freshly rebuilt SDK/formats artifacts. Outputs reversed as intended:
```
HTTPJSON empty successful DELETE: Ok(())
HTTPJSON direct SSE >10000 nodes accepted: Some(Err(SerializationError { message: "A2A JSON structure exceeds limits" }))
SDK probe complete True
canonical A2A default export leaked copied secret: false
export probe exit 0
```

Source recheck: bounded HTTP+JSON DELETE success path retains non-success error handling; direct SSE invokes the same node/depth validator as JSONRPC before typed decode; patch version conversion is explicit and rejects unsupported versions; canonical A2A specifications use ExportPrivacy.screen_json; test hooks return no cleanup mock; discovery Stop invalidates local epoch and resets pending before posting cancellation. Vendor patch notes document both SDK fixes. No new material issue found in sibling current/legacy RPC/direct SSE/body paths or config/canonical source privacy path.

No source edits performed by reviewer; only this review evidence file. Original pre-fix findings remain above as historical evidence, superseded by this approval.

## Concrete GUI follow-up — raw-source interpolation

Root browser QA observed discovery returning Unsupported or unresolved variable after preserving legacy source while switching to1.0. Read-only diagnosis confirms an additional medium issue at `crates/core/src/interpolation.rs:173`: generic replace traverses raw A2A params_source/card_source before existing restoration at lines234–238. MCP neutralizes its opaque sources before replace at lines122–124; A2A does not. The generic template scanner rejects literal `}}` at lines51–55, so a compact nested JSON draft/card or unresolved invocation-only template aborts session/discovery preparation even though typed invocation resolution is separate. `crates/server/src/a2a.rs:221` calls prepare_live before converting request into GET; therefore card discovery processes unrelated invocation/card source. Version switching itself does not choose a params decoder here.

Root notified with precise cause and narrow fix: neutralize A2A raw source fields before generic replace while retaining exact existing restoration and the invocation's resolve_grpc_source validation. This new finding awaits root implementation/fresh verification; earlier six-findings approval remains valid for its frozen scope.

### Raw-source fix recheck

Additional seventh finding resolved. Source now neutralizes A2A params_source/card_source before generic replace, retaining exact restoration. Fresh `cargo test -p moleapi-core preparation_preserves_raw_a2a_json_sources_and_only_resolves_transport_fields --locked` exited0: one nonempty regression passed. Reviewed body includes compact nested JSON, literal `}}`, unresolved message/card-only templates, and URL/header variable resolution; asserts exact original source retention. Invocation still performs typed JSON resolution and budget validation in protocols/a2a.rs. Scoped backend approval includes this correction. Root reported autosave-before-discovery GUI correction separately; new-request frontend regression/browser gate remains root-owned until fresh evidence.
