# SOAP backend implementation report — 2026-10-03

Backend implementation is complete and frozen for fresh review. All changes are in the
shared application worktree. This agent made no commits/pushes, packaging changes,
Docker/system-service changes, or changes to the main checkout. Root owns the UI,
interchange integration, full workspace final verification and delivery.

## Actual behavior and contract

`Protocol {kind:"soap",version:"1.1"|"1.2",service,port,operation,action}` uses
RequestSpec.body as the authoritative XML draft, POST/text body. Empty selection
strings permit standalone raw SOAP. Incomplete XML saves; execution parses/validates
XML and envelope/version/Body/order/method/content-type/action after pre-script edits.
SOAP1.1 requires text/xml + one quoted SOAPAction; SOAP1.2 requires
application/soap+xml with matching action and no SOAPAction. Defaults are ready
before pre scripts; user header edits must remain valid. Ordinary shared headers,
query parameters, Basic/Bearer auth, environments/data/local/temporary variables,
finite pre/post scripts/tests, timeout and verify_tls settings run through existing
Rust HTTP/DNS pinning/network-policy code. SOAP redirects are limited to same-origin
307/308, preserving POST and preventing XML/credential transfer across origins.

Scoped XML values interpolate only data and attributes using mature quick-xml events;
scoped values are escaped and cannot become markup. Namespace URIs cannot be
variable templates. Qualified xsi/xml/other attributes remain qualified. The saved
body/source is never rewritten by execution. History uses the unresolved saved name.

Specification is `{kind:"wsdl",dialect:"wsdl1.1",source:JSON-string}` with source
`{entry_file,files:[{path,content}]}`. Original file content, paths and import
references remain canonical. wsdl0.1.3 supplies definition/service/port wrappers;
its mature roxmltree nodes provide missing SOAP extension/import metadata;
xsd-parser1.5.2 supplies real XSD models. No compile-time-generated client or
custom XML/WSDL/XSD grammar implements runtime import. Virtual imports must be
supplied; no filesystem reads or automatic resource fetches occur. Imported
namespace/type mismatches, ambiguous service/port/operation names and duplicate
schema type/element symbols reject explicitly.

Owner-bound Rust routes used by standalone HTTP and native IPC:

- POST /api/soap/import `{workspace_id,name,source}` -> `{specification,schema}`.
- POST /api/soap/import-url `{workspace_id,name,url,timeout_ms?,verify_tls?}` -> same.
  Explicit checked GET only, UTF8/bounded body, no redirects, no URL userinfo,
  default30s/TLS verification true. Missing dependencies need a supplied bundle.
- Import routes optionally accept environment_id,variables,data,locals for private
  scope screening. Candidate names/sources containing private data are withheld,
  including XML numeric-entity representations.
- POST /api/soap/schema `{workspace_id,specification_id}` -> owned original spec/schema.
- POST /api/soap/template `{workspace_id,specification_id,service,port,operation}` ->
  `{template,fields,action,version,address}` or explicit unsupported-template error.

Schema is services[{name,ports[{name,binding,version,address,operations[{name,action,
style,use,binding_supported,input_elements:[{namespace,name}],template,fields,error?}]}]}].
Fields provide name/namespace/type_name/optional/repeated. Qualified generated tags
use explicit `m:` prefixes so anonymous complex children honor unqualified schema
form defaults. Local ref occurrence flags survive global element resolution.
Standalone raw envelopes remain available; selected WSDL invocation validates
service/port/operation/version/action and ordered payload QNames even if its complex
schema cannot produce a template. Unsupported RPC/encoded/part-filtered bindings
cannot execute as a selected WSDL operation.

Response adds backward-compatible optional/default-None soap_fault:
`{version,code,reason,detail,actor?}`. SOAP1.1/1.2 Fault inspection follows mature
namespace-aware XML nodes. HTTP200 and500 Faults supplement the actual HTTP status,
headers and raw body; target401 is an actual target response. Truncated/invalid XML
stays an actual response without an invented Fault/status.

## Privacy helpers

Exported core helpers:

- redact_soap_xml(&str)->Result<String>: screen payload credential elements/attrs;
  preserve qualified attributes and namespaces; malformed/over-budget XML rejects.
- redact_soap_source_xml(&str)->Result<String>: screen source export clones while
  retaining declaration names/types/namespaces; mask sensitive XSD default/fixed
  values, literal credential payload/extensions and URL userinfo/private query
  values in location/schemaLocation/href/url/endpoint attributes.
- soap_xml_data_values(&str)->Result<Vec<String>>: mature decoded text/attribute/
  namespace values for scoped privacy checks, including numeric entity spellings.
- escape_xml_value(&str)->String: mature XML escape form for generic private patterns.

Canonical sources/drafts remain unchanged. Default SOAP history omits Fault metadata,
redacts literal credential XML, withholds malformed/opaque XML and withholds decoded
XML that contains known private values. Active actual responses remain available.
Root formats owns default/explicit export policy and applies the core XML helpers.

## Bounds and explicit limits

| Item | Enforced bound |
| --- | --- |
| Virtual bundle | 32 files, 2MiB aggregate UTF8 file content |
| Paths | <=256 chars; relative / segments; ASCII alphanumeric,_,-,.; no empty/./../absolute/colon/backslash |
| XML document | 5MiB, <=20,000 nodes, depth64; DTD/external entities disabled |
| Source bundle XML | <=20,000 aggregate nodes, <=64 schemas/import directives |
| Global schema components | <=1024 |
| Services/ports/operations | <=64 services,64 total ports,128 total operations |
| Generated template | <=256 fields, traversal depth16, <=1MiB each |
| Aggregate template output | <=1024 fields and2MiB |
| Required repeated samples | minimum occurrence count <=32; optional/repeated sample is editable |
| Inherited source namespace visits | <=100,000 before DOM/model projection |
| Source names/namespaces | name256, namespaceURI2048/prefix128, QName512, literal/action4096, address8192 |

Supported templates include document/literal WSDL1.1, imported/embedded schemas,
namespace-qualified/unqualified elements, anonymous/named complex sequences/all,
simple restrictions/enumeration/base samples, refs and optional/repeated fields.
Samples are structural editing aids, not a proof of complete XSD instance validity;
restriction facets can need user edits and are enforced by the actual target service.
Explicit errors cover WSDL2/non-SOAP/non-HTTP bindings, RPC/encoded/parts, named
schema groups/choice/derivation/attributes/wildcards/abstract/substitution/identity
constraints/alternatives, list/union simple types, chameleon includes/redefine/
override/local XSD1.1 targetNamespace and recursion/size limits. WS-Security,
advanced common auth/client certificates and XPath extraction are not claimed in
this slice; shared future work owns them.

## Libraries and fixture provenance

wsdl0.1.3 distributed source at VCS7a197342e2360182646fdec72d31c8eb6567b493 is
vendored with its original Apache-2.0/MIT manifest. Upstream declares no authors,
year or copyright notice; none was invented. Full standard Apache2/MIT texts and
attribution are in vendor/wsdl/LICENSE-APACHE, LICENSE-MIT and PROVENANCE.md.
Only nightly feature removal, equivalent stable first-match/error-propagating
iterator loop, and upstream lifetime-warning suppression are patched. XML grammar
and WSDL wrappers are unchanged. Other new mature crates are roxmltree0.18.1,
quick-xml0.38.4, xmltree0.11.0 and xsd-parser1.5.2/xsd-parser-types0.2.1. xmltree is
used to construct new templates/schema projection, not to parse/rewrite actual
request or export XML (its qualified-attribute loss was caught and avoided).

Real SOAP fixture is Spyne2.14.0 + lxml6.0.2, using actual SOAP1.1/1.2 service
serialization and XSD validation. Dedicated disposable venv requirements and a
current-Python six/legacy-cgi compatibility alias are documented under
crates/server/tests/fixtures/soap. No custom SOAP service/parser was substituted.
Actual fixtures remain running on18897 and18898, bearer fixture-token, GET /?wsdl,
Echo(name,optional/repeated Item), name=fault -> real Fault500. Golden WSDLs were
fetched from these services; imported WSDL/XSD variants were generated with lxml.
A new process on18909 also proved the documented unmodified-dependency startup
and SOAP1.2 invocation; it was stopped afterward. Existing fixture/API ports were
untouched by this agent.

## Verification and review

Fresh focused command:
`cargo test -p moleapi-server --test soap --no-default-features -- --include-ignored --nocapture`
passed10/10, including actual Spyne1.1/1.2 action/auth/success/Fault500/target401,
Fault200/raw-status preservation, Basic auth, actual envelope pre hooks/post tests,
scoped XML escaping, history literal/private numeric-entity screening, private
source screening, owner/source/template restore, selected QName guard, hosted
private-network rejection, drafts, imports/bounds/RPC/encoded/recursion, qualified
attrs, inline form defaults/local refs and same/cross-kind duplicate symbols.

Strict all-target workspace Clippy with no default features passed after the
final declaration-once decoded privacy/source namespace-budget fix.
cargo fmt --all --check passed on final source. Root owns the final full
workspace/fmt/Clippy refresh against its concurrent UI/formats changes. Earlier
complete workspace run by this agent passed; a later snapshot failed root's newly
added invalid formats fixture, since corrected by root. Root independently reported
fresh163passed/18ignored workspace, strict Clippy/fmt,73frontend/build prior to the
final decoded history privacy diff. This report does not claim that earlier full
run contains the final privacy change.

Independent reviewer verified namespace/ref/duplicate fixes and final scoped
privacy bounds:10focused tests plus its own decoded numeric/comment-split/hex
attribute/namespace probes. The119,793-byte/2,000-declaration/19,000-child helper
probe fell from11.12s to277ms (~40x) by visiting actual namespace declarations once
through quick-xml instead of repeatedly cloning inherited bindings. Source
projection rejects over100,000 inherited namespace visits before DOM/model work.
No material unresolved findings remained in its scoped follow-up. Backend source
is frozen; no additional feature or distribution work is underway.
