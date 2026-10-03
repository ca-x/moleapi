# SOAP UI and interchange review

Authority: `docs/specs/soap-client.md`, `docs/superpowers/plans/2026-10-03-soap-client.md`. Review baseline: `89f73c6`; reviewed current unstaged SOAP frontend and formats changes. Backend work was concurrent, so Rust inspection was restricted to formats and the core XML-helper DTO/contract.

## Verdict

Spec: gaps remain in canonical-source default-export privacy and bounded native file loading. Quality: changes need the two fixes below before this SOAP slice is considered complete. Dedicated editor uses mature React/Radix/CodeMirror XML language tooling; imports retain original canonical files; manual SOAPAction is editable; ordinary HTTP options/scripts/send and actual response/Fault rendering are integrated.

## Findings

### Important / P1 — WSDL/XSD content escapes default-export screening

Location: `crates/formats/src/redact.rs:122`–`127` (canonical specification traversal), with `redact_value` handling XML content as an uninterpreted scalar.

A canonical WSDL source is JSON containing `files[].content` XML strings. The generic JSON/YAML traversal does not parse those XML strings, so default MoleAPI export retains literal source credentials and credential-bearing WSDL endpoint URLs. New SOAP payload redaction only screens request/example bodies and cannot close this source leak.

Exact source-file content for a regression test:

```xml
<wsdl:definitions xmlns:wsdl="http://schemas.xmlsoap.org/wsdl/" xmlns:soap="http://schemas.xmlsoap.org/wsdl/soap/" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:tns="urn:service" targetNamespace="urn:service">
  <wsdl:types><xsd:schema targetNamespace="urn:service">
    <xsd:element name="password" type="xsd:string" default="literal-source-password"/>
    <xsd:element name="token" type="xsd:string"/>
  </xsd:schema></wsdl:types>
  <wsdl:service name="Svc"><wsdl:port name="Port" binding="tns:Bind">
    <soap:address location="https://alice:literal-source-password@example.test/service?api_key=literal-source-token"/>
  </wsdl:port></wsdl:service>
</wsdl:definitions>
```

Place this in `Specification.source` as `{"entry_file":"service.wsdl","files":[{"path":"service.wsdl","content": XML}]}` with kind `wsdl`/dialect `wsdl1.1`, and call `export(workspace, "moleapi", false)`. Both literal source markers currently remain in `output.content`. This repro exercises formats independent of whether the illustrative document is fully importable.

Fix with bounded mature XML source-aware screening of each canonical WSDL/XSD file. Preserve schema nodes, namespace declarations, component/type/name metadata, `token` and `password` element definitions, and virtual import references. Scrub credential-bearing default/fixed literals, literal SOAP/WS-Security credentials, and address URL userinfo/sensitive query values. Applying request-payload scrubbing wholesale to schema definitions is unsafe. An explicit `include_secrets=true` export must preserve the exact original source. Add default-export absence and structural-preservation assertions plus explicit-export byte equality.

### Important / P2 — Native import reads unbounded files before size rejection

Location: `web/src/features/soap/files.ts:49`–`55`.

The Tauri picker checks file count but calls `readTextFile` for all selected paths in parallel before `checkSoapFiles` enforces the aggregate 2 MiB byte limit. Selecting a very large `.xml`/`.wsdl` file can allocate its entire content in native IPC and the webview, potentially exhausting memory. The browser branch correctly checks aggregate `File.size` first.

Use mature filesystem metadata (`stat`) to reject oversized aggregate selection before loading contents; retain post-read UTF-8 checking, and use bounded reading if the available native API supports it. A mocked native picker/filesystem test should assert that oversized selections never call `readTextFile`.

## Lifecycle and scope observations

Dialog cancel/reopen and owner/workspace/request/environment guards are present; editable source fields lock during validation. Loaded schema is keyed to canonical source plus specification and owner/request scope, with cleanup fences for obsolete schema requests. Template replacement uses the shared Radix confirmation dialog, whose modal behavior blocks user edits while confirming; I did not treat hypothetical programmatic edits behind that dialog as a demonstrated material issue. Existing references are retained rather than removed when source dialogs open. Advanced operation metadata remains available in the operation-definition JSON; unsupported template errors have a visible callout and raw XML remains editable. No custom XPath, WS-Security, or broader researched feature completeness is claimed by this review.

## Independent verification

Executed from `web/`:

- `npm test -- --reporter=dot`: 18 test files, 69 tests passed.
- `npm run typecheck`: passed (`tsc -b`, exit 0).

Read-only source review; only this report was written. No browser session, Docker, commits, pushes, or Rust implementation changes were made. Browser interaction/a11y/narrow-layout evidence remains the root agent’s separately owned acceptance work. Findings were sent to root during review; root owns fixes and scoped verification.

## Scoped fix review

Reviewed only the two reported findings and their fixes, plus the directly related source-attach selection reset.

- P2 **ADDRESSED**: shared `readBoundedTextFiles` checks all native `stat` sizes before opening files, reads sequentially with an aggregate/per-file budget and one-byte overrun detection, performs streaming fatal UTF-8 decoding, and closes handles in `finally`. SOAP and protobuf importers use it; desktop capabilities grant stat/open/read. Three covering tests verify preflight rejection, post-stat growth rejection with close, and split UTF-8.
- P1 **implementation addressed; verification not yet passing**: formats now runs bounded mature `redact_soap_source_xml` against each WSDL bundle XML content, preserves namespace/component identifiers, scrubs credential defaults/fixed values and address URLs; explicit export bypasses transformations. The covering test currently fails at `crates/formats/tests/interchange.rs:430` with `Port binding missing` when reimporting the illustrative incomplete WSDL. Root notified to make the test fixture valid or deserialize the export directly for structural assertions.
- Source attachment also resets old service/port/operation/action; its focused hook test passes.

Independent rerun: `npm --prefix web test -- --reporter=dot` passed 19 files / 73 tests; `npm --prefix web run typecheck` passed. `cargo test -p moleapi-formats --test interchange soap_exports_screen_xml_values_and_preserve_schema_definitions_and_explicit_sources` failed solely at the incomplete-fixture import described above. No source edits.

### Final scoped verification

Root replaced the incomplete canonical test document with the real complete Spyne SOAP 1.1 WSDL fixture plus injected credential fields/address. Independent rerun of `cargo test -p moleapi-formats --test interchange soap_exports_screen_xml_values_and_preserve_schema_definitions_and_explicit_sources` passed: 1 test, 0 failures. This verifies default source/payload secret removal, schema component/type preservation, qualified XML attribute preservation, exact explicit source/body restoration, and malformed payload withholding.

P1 **ADDRESSED**. P2 **ADDRESSED**. Both reported findings are closed; no new material breakage found in their fix diff. Scoped spec and quality review approved. This scoped approval does not replace root browser acceptance or the independent backend review.

## Additional scoped known-private XML screening review

Independent `cargo test -p moleapi-formats --test interchange` passed all 17 tests, including numeric-entity plain-field withholding and prior source/payload/explicit preservation. Aho-Corasick pattern compilation is bounded to 4 KiB per pattern and 16 KiB aggregate, with conservative withholding on budget/build failure; XML parsing and extraction are bounded by the core parser.

Residual material gap identified in the new fix: `crates/core/src/soap.rs:1199` `soap_xml_data_values` only collects decoded text, attributes, and namespace URIs, while `transform_xml` preserves XML comments and processing instructions. A known-private value `a&b` placed in `<!--a&b-->` survives field-name redaction and is never passed to the matcher. Include comment/PI data in default screening or omit comments/PI in default export clones; preserve explicit sources untouched. Root notified.

Conservative matcher limitation: matching identifier attributes and namespace URIs can withhold whole canonical source documents for unrelated short secrets that happen to occur in schema metadata. Default definition preservation therefore holds only for documents passing screening; accurate withholding warnings matter. This is conservative privacy behavior, not an additional exposed-secret finding.

### Final comment/PI scoped recheck

Root adds a direct Aho-Corasick match over safely transformed raw XML before decoded-value screening. This catches literal known-private values in retained comments/processing instructions; decoded screening still catches numeric-entity encodings in ordinary text/attributes. The export dialog now accurately states that private or unprocessable XML may be blank and directs full backups to explicit include-secrets export.

Independent `cargo test -p moleapi-formats --test interchange soap_default_export_withholds_decoded_private_entities_in_plain_data_fields` passed (1 test / 0 failures), including numeric entities, comments/PI, and exact explicit body preservation. Residual comment/PI finding **ADDRESSED**; no new breakage found in this scoped diff. All UI/formats findings reported by this reviewer are closed. Core decoded-extraction namespace performance bounding remains the independent backend reviewer/implementer’s ownership and is outside this scoped approval.
