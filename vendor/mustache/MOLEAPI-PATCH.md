# Mustache0.9.0 parser access

Upstream: https://crates.io/crates/mustache/0.9.0, https://github.com/nickel-org/rust-mustache. Original MIT/Apache-2.0 license and source are retained.

One additive `template_partials` function exposes the existing parser's partial-name result without calling the compiler or its filesystem loader. Parsing, delimiters, sections and token semantics remain upstream code. MoleAPI validates partial paths before handing templates to the embedded OpenAPI Generator; rendering remains its mature Java Mustache engine.

The additive parser section limit bounds this entrypoint to64 nested sections before nested token construction/cached section copies. The original compiler defaults to an unlimited section count as upstream did. A new parser error reports the bounded-entrypoint limit; grammar/rendering are unchanged.
