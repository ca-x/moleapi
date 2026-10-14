# Native Kotlin/Dart/Swift model consumers

Parent: milestone2 / MODEL-GENERATION-COVERAGE.md / code-generation.md.

Generate fresh explicit3.0.3/3.1.0 model-kotlin/model-dart/model-swift fixtures through
the capped worker and embedded Quicktype26.0.0. Preserve original native Jackson Kotlin,
Dart JSON codecs and Foundation Codable models and default renderer options. Write
actual build/run drivers before testing: decoded and constructed recursive Pet/Status
models, zero, nullable native value, Unicode/quotes/ampersand/plus and invalid enum.
Require explicit tool paths, lock/record dependencies, verify original source hashes,
and run Dart analyzer/AOT and Swift release/Kotlin JVM compiler. Product never installs
these tools or runs consumer programs.

Track actual upstream nullable serialization policies explicitly: default Swift Codable
and Kotlin Jackson NON_NULL omit nil-valued optional note; Dart emits null. These default
models do not distinguish absent and explicit-null input after decoding. Do not patch or
replace mature emitters to make a serializer claim they cannot provide. Optional/null
presence fidelity and schema validation remain required separate scope in the parent
module; a native nullable value case proves that value conversion only. Existing four
native-language checks still require emitted explicit null for their own tested policies.

After all consumer/test code is written, run6 actual cases and existing native model
regressions plus generation/model-API/format/lint gates. Update precise catalog/evidence
and integrate verified changes independently of pending Objective-C Apple RPC jobs.
Other6 model runtimes, SQL execution, other libraries/platforms/options/dependencies,
advanced schemas/resources/constraints, SDK/server and release scope remain open.


Verified2026-10-15: six native cases passed actual decode/construction/zero/recursive/
enum/nullable-value/Unicode/unknown-enum checks; generated source pairs match and
remain unchanged. Kotlin/Jackson default omits nil properties, Swift Codable omits
nil properties, Dart emits null; the exact policies are asserted and recorded.
No emitter patch/replacement was necessary. Dart analyzer/AOT and Swift release
builds passed; locked Kotlin2.4/Jackson2.17.3/Gradle8.14.5/JDK21 and Dart3.13.5/
Swift6.4.1/Linux tool/runtime/hash evidence retained in isolated QA. Four old native
model driver regressions,80 generation cases,1 model API case and scoped format/
strict Clippy passed. Catalog now records12 representative model runtimes.

Ruling: integrate this verified batch by a scoped cherry-pick while native Apple RPC
run38641146152 (94eda9c) is still queued; keep null/absence fidelity and the6 remaining
model runtimes/SQL/advanced generation/full-product scope open. Default native
nullable-value conversion does not prove preservation of original object membership.
