# Native Crystal model construction and enum conversion

Parent: milestone2 / MODEL-GENERATION-COVERAGE.md / code-generation.md.

Use signature-verified isolated Crystal1.22 compiler/stdlib with fresh3.0.3/3.1.0
fixtures. Write consumers before tests for decoded and constructed original native
recursive models, declared Int32 zero, nullable value, Unicode and known/unknown
enum. Confirm actual default constructor/enum behavior first; repair only reproduced
mature renderer seams using Crystal JSON::Serializable, native enum and naming/codecs.
Preserve exact authored enum tags (including tags that differ from generated names),
nonrecursive classes, original user source and other model emitters. Record pinned
source/license/hash and reproducible build patches.

After code/tests, run actual release compiles, roundtrips and scoped regressions/
generation/model API/format/lint; integrate verified changes independently of Apple
RPC acceptance. Crystal declares Int32 range; do not infer arbitrary integer or
schema/optional-null fidelity. Other2 model runtimes, SQL/advanced models/SDK/platform/
product/release requirements remain open.


Verified2026-10-15: actual original enum API compilation fails and native String
property accepts invalid tag. Pinned mature Crystal renderer now emits native enums,
exact PullParser/Builder wire mappings and required-first field initializers; the
string utility escapes literal interpolation syntax. Four release compiler/runtime
core/enum-tag cases pass both dialects, including ready-state/done/state/#{1 + 2},
unknown rejection, recursion/construction/zero/nullable-value/Unicode. Original
emitted files remain unchanged. Signature-verified Crystal1.22/LLVM23.1.1/Linux
package/source/hash/runtime records retained in isolated QA.17 other emitters match
prior bytes;5 native/80 generation/1 model API/format/strict Clippy regressions pass.
Catalog records16 runtimes; target retains Int32 and native optional-nil omission.

Ruling: integrate this verified independent renderer/driver batch with scoped cherry-
pick; preserve missing/null/scalar/range and all wider module requirements. Apple
run38648919858 reached real ARM source compilation but failed Swift compatibility
linking from Protobuf's Swift additions; Xcode mixed-language linkage is fixed only
in the pending QA branch and a new native run has been dispatched. No Apple runtime
or full-goal completion claim follows from these local Crystal results.
