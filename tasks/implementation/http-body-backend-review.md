# Independent HTTP body backend review — 2026-10-05

## Follow-up review of the five fixes

**Final focused metadata/privacy pass:** all findings recorded in this report are resolved in the current source snapshot. `screen_bounded` uses mature matcher spans and checks prefix/replacement/tail sizes before append; names and filenames have 512-byte limits and valid `[REDACTED]` fallbacks. Screened multipart text consumes a cumulative 5 MiB budget; oversized serialized sources conservatively withhold text and remain reselectable structured data. The private-`x` repeated-name/filename export/import regression directly covers the last reported failure. No definite important remaining issue was identified in these edited paths on this focused source pass. Parent-reported 29 interchange test results and pending broad tests/Clippy/build were not independently executed by this reviewer; native/browser/platform/live evidence and parent parity remain separate.

The five findings below are resolved in the current implementation snapshot. Dynamic structured sources are screened independent of concrete mode and external formats reject dynamic modes; the original default-byte leak is closed. Interpolation now shares the request expansion budget, subtracts decoded file/dormant-text costs, bounds each active text/name/MIME before appending and uses an 8 MiB bounded JSON writer. Disabled MIME permits retained placeholders without requiring inactive interpolation. Postman empty file arrays retain an unselected placeholder; binary MIME is exported/imported through Content-Type. Export preflight validates structured DTOs before the internal typed parse assertions without introducing full URL validation into native backup. No definite arithmetic underflow, unbounded expansion or new parse panic was identified in those fixes on source inspection. Parent's passing fixtures/full suite are context, not rerun evidence from this reviewer.

### Remaining P2 — Privacy-budget withholding produces an unimportable multipart export

**Latest focused pass:** the empty-name privacy-budget case is resolved: an empty screened field name becomes `[REDACTED]`, preserving valid IDs and repeated field names. Dynamic multipart sensitive text values now enter copied-value screening. The new native export/import regression source covers the 5000-character private-value case. No new execution evidence is claimed by this reviewer.

**Remaining same-shape P2:** bound metadata after screening as well. `screen_text` substitutes every secret occurrence with `[REDACTED]`; a valid original name or selected filename <=512 bytes can therefore expand beyond its 512-byte DTO bound. Concrete case: saved auth password `x`, multipart name `x` repeated 100 times. The original name is valid; its screened name is 1000 bytes, nonempty, and bypasses the new empty-name fallback. Default native import rejects it. Binary/multipart `file_name` uses the same unbounded screening and has the same failure. MIME is already reset on changes and is unaffected. Replace oversized screened names with one valid placeholder and oversized filenames with a valid withheld value, and cover this native roundtrip regression. This is established from metadata limits and the Aho-Corasick replacement path; no live router reproduction was run here.

Location: `crates/formats/src/redact.rs`, multipart `part.name = data_privacy.screen_text(&part.name)` and `ExportPrivacy::screen_text`.

`ExportPrivacy` deliberately switches to `withhold=true` when a known private value exceeds 4096 bytes or its matching budget is exceeded. In that state `screen_text` returns an empty string, including for public multipart field names. A valid workspace with one enabled multipart row named `upload` and a separate secret variable containing 5000 characters therefore exports the row with `name:""`. Native import invokes `validate_workspace`, whose structured-body validation rejects empty multipart names. The default export is no longer a valid reselectable backup. The export preflight validates the original source before redaction, so it does not catch this transformed invalidity.

Correction: retain a valid opaque placeholder name when privacy requires withholding, or explicitly fail the default export rather than emitting an unimportable source. Add an oversized-private-value native export/import fixture with multipart data. This source-path finding was not reproduced through a live router by this reviewer.

Reviewed current uncommitted body slice against `docs/specs/http-request-bodies.md`, base `daabcb0152c95dfd9c93c77d3422838c96c31d1a`. No implementation edits, commits, Docker or delegated work. Only this report was written. Parent's three actual Axum fixture results are reported context, not independent test execution by this reviewer.

## Definite important findings

### P1 — Templated body mode bypasses default opaque-byte withholding

Locations: `crates/core/src/validation.rs:123-141`; `crates/core/src/interpolation.rs:174-197`; `crates/formats/src/redact.rs:49-86`.

The existing model permits a saved `body_kind` such as `{{mode}}`. Execution resolves that field first and treats its body as BinaryBody/MultipartBody when mode resolves to binary/multipart. Default export instead only recognizes literal `binary`/`multipart`. Generic JSON redaction does not remove the `base64` field. A valid saved request with `body_kind="{{mode}}"`, body `{"file_name":"private.bin","mime":"","base64":"AP9B"}`, and mode variable `binary` therefore uploads decoded bytes normally but exports those opaque bytes unchanged in default MoleAPI output. Postman also follows its raw-body fallback for the unresolved mode and includes the source JSON.

Correction: establish a save/export invariant that structured uploaded data cannot hide behind a dynamic mode. Either explicitly reject this combination or conservatively withhold typed opaque source independent of the unresolved mode. Add default native/Postman export fixtures for templated modes, with explicit private native export retaining the original.

### P1 — Structured interpolation resets expansion budgets for every field

Location: `crates/core/src/request_body.rs:142-162`; `crates/core/src/interpolation.rs:192-197`.

Structured bodies are exempted from generic `replace`'s shared 20 MiB budget. The new resolver calls `interpolate()` independently for every name/text/MIME; each invocation starts a fresh 5 MiB budget. All expanded values are retained, then serialized and parsed again before final aggregate/source/metadata validation rejects them. Thus a small valid body with 64 enabled text rows, each text containing five copies of a ~900 KiB variable, can materialize roughly 288 MiB of text before rejection. The variable itself fits the existing 1 MiB scope bound. Expanding names and MIME fields similarly allocates very large values before their small metadata limits are checked. Hosted execution can amplify this across concurrent requests.

Correction: share a cumulative expansion budget, constrain each metadata field while expanding, and check decoded/body-source aggregate before serialization. Verify a many-parts repeated-variable case rejects during expansion, rather than after constructing the complete oversized body.

### P2 — Disabled multipart MIME templates block otherwise valid execution

Location: `crates/core/src/request_body.rs:59-65,114-125,149-159`.

Save permits template MIME values. Resolution deliberately skips disabled parts, but final execution validation still parses every part MIME with `templates=false`. A disabled text/file row with MIME `{{unused_mime}}` therefore causes `Invalid body MIME type` even though neither its bytes nor MIME would be sent. This defeats the enabled-part control and differs from the demonstrated supported disabled missing-file case.

Reproduction: one enabled ordinary text row plus one disabled file row with `base64:null` and `mime:"{{unused_mime}}"`; save validation passes, execution fails before sending the enabled row. Correction: validate dormant source structure/bounds without requiring inactive interpolation to resolve; active rows retain strict concrete MIME validation.

### P2 — Postman empty file arrays silently drop active missing-file parts

Location: `crates/formats/src/postman.rs:170-205`.

For `type:"file", src:[]`, the importer produces an empty paths vector and adds no MultipartPart. The imported active missing-file row disappears entirely, and the request can execute with remaining text rows instead of requiring reselection. An array containing only nonstring entries has the same behavior. This violates the null-versus-empty/missing-file contract: absence of selected files is not an actual zero-byte file and must not become absence of a required part.

Correction: preserve at least one unselected placeholder for an empty/malformed file path array, or reject malformed input explicitly. Test a normal Postman formdata file row with `src:[]`, including warning and execution refusal until selection.

### P2 — Binary Postman export loses the selected file MIME metadata

Location: `crates/formats/src/postman.rs:343-346`; import `:154-162`.

Binary transport sends `BinaryBody.mime` as Content-Type when there is no explicit header. Postman export writes only `file.src` and leaves headers unchanged; import creates BinaryBody with empty MIME. A binary body with MIME `image/png` and no stored Content-Type header therefore changes to application/octet-stream after Postman export/import and reselection. The filename survives, but its deliberate content type does not.

Correction: represent the binary MIME through Postman's request Content-Type header when there is no enabled explicit override, and preserve that semantic on import. Add a metadata roundtrip fixture alongside the expected withheld-byte/reselection behavior.

## Scope/evidence

Inspected typed body DTOs, source/decoded/encoded limits, interpolation, mature reqwest snapshot encoding and checked redirect replay, missing-versus-empty handling, manual boundary rejection, Postman import/export and default/native privacy, snippet adapter rejection, script pm body mutation, raw-byte execution and server history handling. Snippet generation explicitly rejects unsupported modes rather than emitting source JSON as wire content. Raw byte transport keeps explicit override precedence; webhook replay uses its existing concrete request marker. No definite new panic or selected-file-byte corruption was established in these inspected paths. History intentionally withholds uploaded-mode response body/header/url/test values and does not persist live request updates/logs.

No new tests were run by this reviewer. The source paths above establish the findings; actual router privacy/import fixtures and expansion/resource regressions remain required. Full application/native/browser/CI evidence and the parent capability matrix remain separate; this slice does not establish parity.
