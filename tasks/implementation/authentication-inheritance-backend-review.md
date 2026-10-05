# Independent authentication inheritance backend review — 2026-10-05

## Focused fix re-review

**Final scoped backend approval — 2026-10-06:** all confirmed findings in this review are resolved in the current source snapshot. Postman direct-root conversion now explicitly rejects an inert root (`variables_enabled=false`) with enabled stored definitions, preventing activation of source-only values. The regression source verifies exact native backup and explicit Postman refusal. Earlier standalone-inherit claim remains retracted. No further definite important issue was identified in this final focused pass. Parent reports 285 Rust tests and strict Clippy passing; this reviewer did not rerun them. This approval covers the reviewed inheritance/backend corrections only: separate environment/global/collection interchange bundles, broader auth/capability matrix and platform/distribution evidence remain explicit full-goal gaps.

**STDIO compatibility addendum:** the latest request validation explicitly rejects HTTP authentication kinds for MCP STDIO, including Basic/Bearer that the mature process transport would ignore. Save-time inherit/dynamic selectors remain permitted, while contextual inherited selection receives the same compatibility guard and explicit No Auth remains valid. This matches the authority's requirement to fail incompatible inherited transport auth explicitly. Existing dormant credential fields can still be retained for privacy checks. No definite guard regression was identified on source inspection. Parent reports 286 Rust tests, strict Clippy and binary build passing; this reviewer supplies no new execution claim.

### Script bounds and Postman Runtime compatibility pass — 2026-10-06

The remaining script-expansion finding below is resolved: workspace/collection/request pre/post output uses `screen_bounded(MAX_SCRIPT_BYTES)`, with safe empty fallback. The source fixture covers the repeated-short-token case. Imported ItemGroup variables retain source definitions with `variables_enabled=Some(false)` and private flags; core ancestor scopes and runner source merging skip those definitions. Native None remains enabled. Actual SDK/runtime log `/tmp/moleapi-postman-runtime-qa.log` reports Runtime 7.56.1/SDK 5.3.0 folder-auth inheritance and ignored folder variables. This reviewer read that output but did not execute the experiment. Rejecting executable native folder/wrapped collection-variable exports is consistent with the updated spec rather than claiming the ignored Runtime fields execute.

**Remaining P2 — direct-root export activates inert root definitions.** The new flag applies to Collection generally, including a native root. `export_collection` writes every source variable with `disabled:!row.enabled`, ignoring `collection.variables_enabled`. When a single root qualifies for direct_root, export moves those variables into Postman's executing top-level collection variable list. The guard permits direct_root unconditionally. A valid native root with `variables_enabled=Some(false)` and enabled stored definitions is therefore inert in MoleAPI scopes but executing after Postman export/import. This can also supply an inherited credential template that native execution correctly treated as missing. Reject direct-root inert-definition shapes or provide a verified inactive mapping; preserve full source via native export. Add a root-flag false export fixture alongside the imported-folder false fixture. Wrapped false folders remain safe under the verified Runtime behavior.

No definite remaining script-bound issue was found in this scoped pass. Full gates are parent-running; no new execution/platform/parity claim is made here.

The original parent source-copy finding and Postman root/environment projection finding are resolved in current source. Workspace/ancestor scripts and collection descriptions now use the known-credential matcher; Postman computes direct_root once and guards every collection that remains a folder. Projection now has a shared 4096-scope/65536-row/16 MiB scan budget, checks costs before building scopes and skips empty/literal sources; exhaustion chooses withholding. Full native backups bypass privacy projection. No definite overflow or continued unbounded product construction was found in those precise changes. The standalone-inherit claim below is retracted because strict validation already rejects it.

### Remaining P2 — Script redaction expansion makes default backups unimportable

The new workspace/ancestor script screening calls unbounded `screen_generation_text`. Known-secret replacement can expand a valid original script beyond `MAX_SCRIPT_BYTES` (256 KiB), while default native export does not revalidate transformed scripts. Concrete reproduction: parent bearer token `p` and parent/workspace pre-script `//` followed by 50,000 `p` characters. The original is a valid 50,002-byte comment; redaction produces roughly 500,002 bytes because every secret becomes `[REDACTED]`. Native import rejects it with `Script exceeds 256 KiB`. Request scripts use the same screening path. This repeats the previously corrected bounded-metadata pattern on the newly inherited source surface.

Correction: bound script output while screening, using a safe empty/withheld fallback when replacements exceed the source limit; cover the repeated-short-secret default/private native roundtrip. This is established from matcher replacement and script byte validation, not a newly executed fixture.

Parent reports focused core/formats tests passing in `/tmp/moleapi-inheritance-review-fix-tests.log`. This reviewer inspected the fixes and fixture source without independently executing tests. Full/native/browser/platform/CI and parent capability parity remain separate.

Reviewed current uncommitted slice at base `81ac1a0054190a231ddfaf893f34fa9f9e4e018f` against `docs/specs/authentication-inheritance.md`. Only this report was written. No source edits, commits, Docker, delegated work, or new test execution.

## Definite important findings

### P1 — Default export exposes credentials copied into ancestor scripts/source

Location: `crates/formats/src/redact.rs`, workspace/collection handling preceding request copy-screening.

The collector now knows workspace/ancestor credentials and leaf-scoped resolved values. Redaction clears parent auth and screens variables/request scripts, but never screens `WorkspaceData.pre_request_script/post_response_script`, collection pre/post scripts, or collection description. A parent bearer token `parent-secret` copied into parent pre-script `const credential = 'parent-secret';` remains verbatim in default MoleAPI and Postman export after parent.auth.token is cleared. Workspace scripts have the same path. These are explicitly inherited executable/source surfaces under the spec's default-export parent/copy protection.

Correction: screen known credentials in all inherited script/source layers with bounded output and native schema retention. Add a parent/workspace credential copied into ancestor scripts/description default/private roundtrip fixture. Request-only fixtures cannot establish ancestor protection.

### P2 — Postman environment conflict guard excludes root collections exported as folders

Location: `crates/formats/src/postman.rs`, selected environment conflict check (`filter(|c| c.parent_id.is_some())`) and root export/flatten branching.

Native environment variables override every collection scope, including a root. Postman export projects the selected environment into outer collection variables but only checks conflicts against nonroot collections. Unless the single-root flatten special case applies, each native root becomes a Postman item-group folder; its variables then override the projected outer values. A workspace with root named `Folder` (workspace named `Workspace`), root variable token=`root`, selected environment token=`environment`, inherited root bearer `{{token}}` silently changes effective token after Postman export/import. The current explicit conflict rejection therefore misses this ordinary root-as-folder case. Multiple-root workspaces have the same issue.

Correction: determine which collections actually remain folders in the chosen export shape and reject their conflicting enabled values, including unflattened roots. Verify effective scoped values after roundtrip, not only equality of folder source fields.

### P2 — Unresolved inherit is accepted by context-free execution and sent as no auth

**Retracted after direct re-check:** this claim was incorrect. The actual supported-kind condition admits inherit only through `templates && auth.kind == "inherit"`. Strict materialization already calls `validate_authentication(..., false)` and rejects it, including context-free execute/request_headers paths. This finding is not open and should not drive an implementation change. The initial report inferred the added kind from the diff rather than checking the full boolean expression; the correction is grounded in the exact current source.

Location: `crates/core/src/authentication.rs`, supported kinds and `prepare_authentication` fallback; `crates/core/src/transport.rs`, public execute/execute_bytes.

`inherit` is accepted even with `templates=false`. The server helpers resolve it using workspace context first, but standalone checked core execute/execute_bytes/request_headers have no such context. `prepare_authentication` falls through for inherit and request_headers emits no credential. A request that explicitly asks to inherit is thus silently sent unauthenticated rather than rejected as unresolved. This also leaves an unsafe fallback for future callers that omit the new shared helper.

Correction: reject remaining literal inherit during strict execution/materialization validation, while allowing it during save/export and in the ancestry resolver. Verify context-free public execution refuses inherit without sending, and contextual server execution still handles explicit none/nearest/workspace/default properly.

### P2 — Parent/environment privacy projection multiplies scope construction without a work budget

Location: `crates/formats/src/redact.rs:611-650`, analogous request credential projection loop.

For every leaf collection and every ancestor auth, the new collector reconstructs full VariableScopes twice for every environment for each credential source, including empty and literal strings that require no resolution. Workspace validation bounds collection count/depth but does not bound environment count. The credential matcher budget is checked only after all these loops finish.

A small valid workspace with 1000 unrelated root collections, each containing Basic auth with a literal password, plus 1000 empty environments causes roughly 4,004,000 scope constructions per privacy matcher (token/password × native/hosted × environments including None × roots). Default export builds both ordinary and generation matchers, doubling this work despite all resolved secrets being identical. Hierarchy depth and multiple credentials increase it further. Input byte/individual variable limits do not bound this projection work; there are no network waits or deadlines around these synchronous export loops.

Correction: bypass projection for empty/literal sources, construct each relevant scope once, deduplicate identical projection contexts, and apply a cumulative work budget before building the complete product. Conservatively withhold or explicitly fail when that budget is exceeded. This is a source-established operation-count finding, not a measured latency or memory claim; this reviewer did not benchmark the hostile-size workspace.

## Covered surfaces and limits

Read optional parent/workspace auth model, bounded chain/subtree, validation, nearest source selection and explicit-none termination, dynamic selector resolution, ancestral/native variable merge, pre/post script order, parent credential capture, finite/live/reflection/introspection shared paths, descendant runner overlays, source snippet inheritance, parent/default/native privacy, and actual Postman hierarchy/auth/null/empty folder behavior.

Chain traversal rejects missing parents, cycles/self-parent and depth greater than 16; save validation rejects duplicate collection/request IDs. Root-to-leaf scopes and script execution order are consistent in the inspected runtime paths; runner overlays follow selected ancestry and avoid reusing sibling collection maps. gRPC reflection and GraphQL introspection route through shared contextual execution. Parent JWT decoded key capture includes workspace/collection settings. External unsupported auth/OpenAPI hierarchy mappings reject explicitly. No definite additional stale-owner, ignored-database auth, ancestry mutation, panic, or sibling-overlay leak was established in this read-through.

Parent reports meaningful format/native inheritance and descendant-runner fixtures passing. Those results were not independently rerun by this reviewer. Fresh full tests/Clippy, UI/native/hosted/browser/platform evidence and separate Postman environment export remain pending or separately tracked. This slice does not establish full auth or parent product parity.
