# PHP native nested models and maps

Parent: milestone 2 / PROJECT-GENERATION-COVERAGE.md.

Extend trusted fixture generation with an explicit PHP-only `containers` profile.
Preserve default mature PHP/Guzzle models, serializers, signatures and runtime.
Envelope contains one Pet, Pet arrays, string-keyed Pet maps and integer maps,
required nullable note and optional nullable label in OpenAPI3.0.3/3.1.0.
Write a separate real HTTP consumer before tests: assert native nested Pet types,
Unicode/zero fields, string map keys, required-null presence, optional omission,
typed construction and unchanged JSON on GET/POST; reuse core auth/query/error cases.
Verify generated hashes remain unchanged, preserve dependency locks/tool evidence,
and stop/join the fixture server. Repair only reproduced upstream seams through
existing pinned compatibility templates with explicit override regressions.

Run two actual default PHP consumer cases, generation/API/format/lint gates, then
record exact bounds and merge the verified batch. Empty maps, null-valued map
entries, nested unions, async maps, constraints, other analyzers/frameworks and
other platforms remain separate acceptance cases.


Verified 2026-10-15: the first actual consumer exposed invented absent optional-null
fields. Existing pinned serializer now requires property presence for nullable setters.
Both dialects pass constructed/decoded/explicit-null echoes and missing nullable
required-property invalidity,14 real core/container requests; fresh union regressions
pass14 more.80 generation/12 API cases, actual PHP template override,2 framing cases
and scoped format/strict Clippy pass. Original generated hashes, actual PHP8.5.11/
Guzzle7.15.5/Linux metadata and locks retained in isolated QA.

Ruling: integrate this verified PHP batch with a scoped cherry-pick, leaving the
Objective-C acceptance commit on the development branch until Apple runtime jobs finish
(run38637719998, both macOS jobs currently queued). The native Apple consumer has no
local runtime success claim; delaying that batch does not hold up this independent fix.
