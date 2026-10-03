# Independent SOAP backend review

Review base: `89f73c6`; authority: `docs/specs/soap-client.md` and `docs/superpowers/plans/2026-10-03-soap-client.md`. Scope: Rust SOAP core/server, Cargo dependencies, vendored WSDL compatibility patch, fixture/tests, shared interpolation/transport/privacy/history integration. UI and interchange are separate reviews. Reviewer made no source changes.

## Material findings and fixes

1. Global elements with anonymous complex types incorrectly propagated global element qualification into local children even when XSD `elementFormDefault="unqualified"`. Independent `/tmp/soap_review_probe.rs` showed local `name` had namespace `urn:moleapi:soap`; the first fix corrected field metadata but not serialized XML because xmltree did not emit an empty default namespace reset. The added regression failed independently (7 passed / 1 failed). Latest fix uses mature DOM namespace prefixes for qualified payload elements and preserves the schema form default independently.
2. Referenced elements discarded local `minOccurs` / `maxOccurs` field metadata when delegating to a global element. A reference with `minOccurs="0" maxOccurs="unbounded"` produced `optional=false repeated=false`. Latest fix applies the local reference occurrence metadata after resolution.
3. Duplicate complex/simple schema type declarations silently overwrote earlier definitions, generating an arbitrary template. Independent duplicate `complexType name="T"` probe selected the second declaration without error. Latest fix rejects ambiguous duplicate declarations; cross-kind simple/complex type-name collisions now share the type-definition namespace and are rejected.

## Review coverage

- Mature `wsdl` 0.1.3 wrapper plus roxmltree/quick-xml/xmltree/xsd-parser own XML/WSDL/QName/schema parsing and serialization. Vendored WSDL provenance retains the upstream grammar; its changes remove the nightly feature and replace one unstable iterator operation.
- WSDL 1.1 SOAP 1.1/1.2 binding/action/address and imported virtual WSDL/XSD bundles; explicit guards for WSDL2, unsupported style/use/parts, schema derivation/attributes/choice/recursive templates and chameleon includes. Sources remain original canonical bundle contents.
- Input byte/node/depth/file/import/schema/component/template budgets; DTD parsing disabled; no automatic filesystem/network import resolution. URL imports use bounded checked HTTP, with redirects requiring explicit URL selection.
- Owner-bound candidate/schema/template access, workspace source restoration/CAS attachment and selected operation/message validation before target networking.
- SOAP wire envelope/version/POST/content-type/action validation after pre scripts; checked targets and SOAP redirect preservation/origin guards; shared auth, variable scopes, scripts and post-response tests.
- Actual target HTTP status and XML response retained; structured Fault extraction on HTTP 200 and error responses; target 401 retained as target status.
- XML interpolation escapes data while retaining qualified attributes and namespace declarations; credential-field screening, encoded known-private values and XML-aware history withholding/redaction. Definition redaction preserves XSD names/types and screens schema default/fixed credential values and endpoint credentials.

## Independent verification

Initial frozen source: `cargo test -p moleapi-server --test soap -- --include-ignored` passed 7/7, including existing live mature Spyne fixtures on 18897/18898. First review-fix rerun failed the new namespace assertion, confirming the regression test detects the incomplete fix. Final fixed-source independent command `cargo test -p moleapi-server --test soap -- --include-ignored` passed **9/9**, zero failed/ignored (1.58s test execution), including the two new template/duplicate regressions and real Spyne SOAP1.1/1.2 invocation/auth/Fault coverage. Independent `/tmp/soap_review_probe.rs`, compiled against the rebuilt core rlib, additionally confirmed actual local child namespace `None` and referenced field `optional=true repeated=true`. The duplicate-complex probe now returns the explicit duplicate schema type error.

Verdict: **No unresolved material findings in the reviewed backend surface.** The three findings above were fixed and independently verified. Review scope remains backend; this report does not claim root UI/interchange or whole-workspace checks, Clippy, formatting, packaging, commit or push completion.


## Scoped numeric-entity privacy follow-up

Reviewed new `soap_xml_data_values`, history withholding, candidate source screening and numeric-entity regression after the original clean verdict. Fresh SOAP suite passed 9/9 including live Spyne; independent decoded-data probe confirmed comment-split text `a&#38;<!--split-->b`, hexadecimal attribute values and numeric entity namespace URIs all decode for matching. Candidate imports screen decoded XML values; history retains actual live responses and withholds the stored body when decoded data contains a known-private value.

Material follow-up finding: helper enumerated every inherited namespace on every element, creating a multiplicative workload. Independent `/tmp/soap_privacy_probe.rs` used valid 119793-byte XML with 2000 root namespace declarations and 19000 empty children (within existing node/depth/body budgets); helper alone took 11.12 seconds synchronously. Fixed by enumerating actual namespace declaration attributes once through mature quick-xml events, with the mature bounded roxmltree document supplying decoded text/ordinary attributes. WSDL schema preflight additionally caps inherited namespace inspection at 100000 visits before DOM projection and model compilation. The new pathological input regression checks decoded values and explicit schema-budget rejection.

Fresh independent final-frozen-source command `cargo test -p moleapi-server --test soap -- --include-ignored` passed **10/10**, zero failed/ignored (0.65s test execution). The same independently recompiled 119793-byte probe took **277ms** instead of 11.12s; decoded comment-split text, hexadecimal attributes and numeric namespace URI matching remained correct. No unresolved material findings in this scoped follow-up. Source edits were owned entirely by the implementer; reviewer only updated this report.
