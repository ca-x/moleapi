wsdl 0.1.3, https://crates.io/crates/wsdl/0.1.3
Upstream https://github.com/DrChat/wsdl-rs, registry VCS revision
7a197342e2360182646fdec72d31c8eb6567b493.
The distributed Cargo manifest declares `Apache-2.0/MIT`; retained verbatim.
Neither registry package nor upstream repository ships a separate license text.
Complete standard Apache2 and MIT texts are supplied alongside this attribution.
The registry and original Cargo manifests supply no author, copyright holder, or
year; none is inferred or fabricated. The Apache appendix placeholders are
part of its standard explanatory boilerplate, not an ownership assertion.

Minimal stable-compiler compatibility patch:
- Remove nightly-only `#![feature(try_find)]`.
- Replace the single Iterator::try_find with a stable loop with the same
  first-match/error propagation behavior.
- Suppress upstream mismatched_lifetime_syntaxes warnings on modern stable Rust.

No XML/WSDL grammar, namespace lookup, parser, or protocol behavior was replaced.
MoleAPI uses the wrapper for WSDL definitions/services/ports and its mature
roxmltree document nodes for SOAP extension metadata and imported QName lookup.
