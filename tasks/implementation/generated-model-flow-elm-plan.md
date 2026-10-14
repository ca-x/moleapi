# Native Flow and Elm model consumers

Parent: milestone2 / MODEL-GENERATION-COVERAGE.md / code-generation.md.

Use fresh3.0.3/3.1.0 fixtures, actual Flow checker/Babel annotation stripping/Node
and Elm compiler/standard JSON packages. Preserve mature codecs and exact generated
sources. Write drivers before tests for decode/construction/recursive/enum/zero/null/
Unicode, unknown enum rejection; Flow additionally checks malformed typed construction.
If actual Elm compilation reproduces recursive-alias/decoder problems, repair only
the mature Elm renderer using its existing cycle analysis and native tagged types/lazy
decoders. Keep source/version/license/digest and reproducible build patch records;
nonrecursive alias APIs must remain unchanged. Test public wrapper construction.

Exact tools/dependencies/source/hash/dialect/evidence remain isolated QA; product is
host-free and never downloads/runs these compilers. After implementation/test code,
run4 actual cases plus native regressions, generation/model API/format/lint gates,
record exact compatibility bounds and integrate verified changes. Other4 model
runtimes, SQL execution, advanced schemas/resources/options and full SDK/platform/
product/release scope remain open; a model conversion is not universal validation.


Verified2026-10-15:8 actual core/date/flat/collision cases passed on Node26.11.1,
Flow0.335/Babel8.0.7/flow-strip-types8.0.1 and Elm0.19.3/standard JSON packages.
Original Flow failed4 generator checker errors plus a QA fs library issue; modern
Flow adaptation fixes the mature emitter, and the QA loader stays untyped outside
original interface checking. Four Flow cases each produce exactly2 expected negative
ID/enum errors. Original/minimal cyclic Elm aliases crash, nonrecursive control passes;
wrapped/lazy Elm output and naming namespace fix pass both dialects plus nonrecursive
construction and StatusValue collision.16 other emitters match original generated
bytes and5 baseline runtimes pass.80 generation/1 model API/format/strict Clippy pass.
Artifacts identify the two patched targets as26.0.0+moleapi.1; source/license/digest
records, exact package/tool/source/runtime/negative evidence retained in QA.

Ruling: preserve mature conversion behavior; missing/null and optional nonnullable
serialization fidelity remain parent work rather than a universal schema claim.
Integrate this verified independent patch by scoped cherry-pick; Objective-C RPC
run38641146152 is still active (Intel generating after successful server build, ARM
queued at last check). Neither platform scope nor the overall goal is complete.
