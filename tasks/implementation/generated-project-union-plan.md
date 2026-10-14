# Tagged SDK Union Runtime Plan

Parent: `docs/specs/generated-project-runtime-fixtures.md`.

1. Add a separate trusted union profile with a referenced Cat/Dog oneOf, explicit discriminator mappings, distinct required fields and tag enums. GET both variants and POST the decoded generated models back without custom serialization.
2. Author default JavaScript/Python/Go/TypeScript/C# consumers that assert the selected subtype and its Unicode/boolean data, plus exact wire roundtrip and existing core/privacy-safe fixture calls. Finish code before checks.
3. Run3.0/3.1 cases, diagnose concrete mature-generator failures, fix through supported engine options/templates where appropriate, and retain explicit target limits. Run generation/lint/format gates, record exact tool/dialect evidence and the newly observed older-commit platform CI results, then commit/merge/push verified work.

Findings/fixes: JavaScript's mature oneOf wrapper now binds its existing toJSON lexically, avoiding the reproduced SuperAgent copy/prototype failure. C# reuses successful mapped-subtype discriminator validation for the redundant parent field parse/required check while retaining other/invalid-field checks. Both changes are pinned Apache-2.0 upstream partials, preserving explicit override priority.

Verification:10 default client/dialect union cases passed (70 HTTP calls); C# missing/unknown tags and missing selected-child fields reject; affected JavaScript binary/C# multipart regression cases passed; generation60 passed/11 configured external-tool cases ignored;5 explicit compatibility-template override tests passed; scoped strict lint/format/diff gates passed. Broader union/constraint/dynamic and ecosystem/browser/platform/install requirements remain open.
