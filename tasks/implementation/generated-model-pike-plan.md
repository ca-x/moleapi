# Native Pike nested model conversion

Parent: milestone2 / MODEL-GENERATION-COVERAGE.md / code-generation.md.

Use an explicitly selected isolated Pike runtime and fresh3.0.3/3.1.0 fixtures.
Write real compile/run consumers before tests for original generated Pet/Status
types, decoded/constructed recursion, zero, explicit JSON null and Unicode. Require
native nested Pet objects rather than retained JSON mappings. Investigate actual
enum/nullable/optional behavior before claims, repairing reproduced defects only in
the mature pinned Pike renderer with exact source/license/hash/build records.
Existing Pike array/map conversion should serve nested properties, not new parsers.
Original user sources and native JSON codecs remain in control.

After implementation/test code, run both native cases, related container/rejection
cases if required by the reproduced seam, existing model regressions and generation/
owned API/format/lint gates. Record exact runtime/library/package/hash evidence.
Product never downloads or runs this interpreter. Other3 model runtimes, SQL
execution, broader schemas/options/platform/ranges and full product scope remain open.


Verified2026-10-15: original interpreter compiled the generated source but failed
actual native recursion and accepted invalid enum. Mature renderer now delegates
property/class/enum/top-level conversion with native map and preserves JSON null/
UNDEFINED. Four actual3.0.3/3.1.0 core/container cases pass, including native object
checks in direct/array/map/top-level values, zero, UTF8 and container enum rejection.
Pike8.0.1956/Ubuntu8.0.1956-4 package matches official index SHA256; isolated libcrypt
package signature verifies with Arch keyring. Actual library/master/module/include
launcher config, package/source/hash/platform/runtime and stderr version recorded.
17 unrelated emitters match original bytes;5 runtime/80 generation/1 API/format/
strict Clippy regressions pass. Catalog now records15 representative model runtimes.

Ruling: integrate this verified renderer/driver batch via scoped cherry-pick while
corrected Apple RPC run38648919858 remains queued/building; keep all remaining
model/SDK/platform/product scope active, including complex union and null/absence
fidelity. Native container conversion does not establish universal schema validation.
