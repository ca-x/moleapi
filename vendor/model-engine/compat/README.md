# Quicktype26.0.0 model renderer compatibility patches

Upstream: quicktype-core26.0.0, source and Apache-2.0 license from the locked npm
package. The original license remains tools/models/quicktype-core-LICENSE and in
the bundled model-engine licenses. patches.json records exact original and patched
SHA256s per file. apply-patches.mjs verifies package/version/hash, applies only these
pinned diffs with the standard patch utility, checks result hashes and is idempotent.
Build applies patches before ESM imports and bundles the mature host-free engine;
no patch utility/compiler/runtime dependency is introduced into product generation.

## Modern Flow

The actual Flow0.335 full checker rejected default emitted mixed, prototype method
rebinding and Date-constructor comparison after control-flow refinement. FlowRenderer
now maps prefer-unknown to the modern unknown type; internal own-property checks use
Object.hasOwn (ES2022), preserving shadow-safe own-key checks. Date dispatch uses
Object.is identity before object refinement, preserving the mature parser and numeric
Date rejection. These JavaScriptRenderer changes apply only to Flow; JavaScript and
TypeScript outputs remain byte-identical. Public model fields retain type checks;
wrong ID and enum construction produce exactly2 native checker errors per case.
Babel strips annotations only; it does not replace codecs or rewrite runtime models.
Actual Node decode/construction/unknown-enum/native-date/invalid-date cases passed.
Legacy Flow/JavaScript runtimes require separate evidence; this output uses ES2022.

## Recursive Elm

Original recursive Pet aliases crash actual Elm0.19.3 compilation, including a minimal
recursive alias control; the same compiler builds a nonrecursive control successfully.
The renderer now reuses Quicktype's cycleBreakerTypesForGraph through the existing
renderer hooks, wrapping only selected cyclic classes in native tagged record types.
Nonrecursive classes retain aliases. Class-reference decoders use standard Jdec.lazy;
codecs, property naming, enum decoding and JSON pipeline remain upstream. Wrapped
decoder construction and encoder unwrapping use the original record fields; constructors
are exported. Arguments use a separate mature naming namespace that forbids global
names, fixing actual shadowing of the status decoder and checked StatusValue collision.

Migration for recursive classes: construct Pet { ... } and unwrap with (Pet fields).
Plain recursive aliases had no valid compiled API. Nonrecursive record construction
remains unchanged and passed a separate real compiler/runtime case. General mutually
recursive unions, recursive non-object containers and advanced schemas/options still
require separate acceptance; do not infer blanket Elm/schema compatibility.

Both changed targets identify quicktype-core@26.0.0+moleapi.1 in generation artifacts.
Other16 Quicktype model target outputs match their original sources byte for byte.


## Pike native property and container conversion

Actual Pike8.0.1956 compiled the original source but retained JSON mappings for nested
Pet properties and accepted an unknown Status value. The original PikeRenderer SHA
and patched SHA are recorded in patches.json. Property conversion now invokes the
mature generated model/enum converters, reusing Pike's native map operation for
array/map members. Top-level container conversion uses the same expression rather
than the old one-level class-only dispatch. Native JSON encoding, member naming,
class shapes, primitive representation and nonnullable union behavior remain upstream.

Nullable properties preserve native Standards.JSON.null and UNDEFINED; optional
missing fields remain UNDEFINED rather than attempting model conversion. Enum
converters compare the existing generated constants and reject unmapped values
without exposing the source payload. A dedicated actual consumer requires native
nested model objects in direct/array/map/top-level containers and verifies zero,
Unicode, explicit null and enum-array rejection. Standard UTF8 conversion is performed
at the QA file/console boundary; the model codec itself is not replaced.

Both3.0.3/3.1.0 core/container cases pass.17 other model target outputs remain
byte-identical. Pike artifacts identify26.0.0+moleapi.1. Optional/null membership,
complex union/scalar validation, other input/container shapes and Pike/runtime/
platform ranges require separate acceptance within the full module.
