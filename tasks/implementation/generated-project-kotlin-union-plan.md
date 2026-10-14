# Kotlin Tagged Union Runtime Plan

Parent: `docs/specs/generated-project-runtime-fixtures.md` and tagged SDK union plan.

1. Inspect default Kotlin jvm-okhttp4/Moshi emitted oneOf models and run actual3.0/3.1 tagged model decode/roundtrip consumers. Keep the default library/build and the same wire requirements; do not mistake flattened fields for typed variant support.
2. Repair actual defects using mature engine options/templates/serializer adapters where possible. If a library/serializer combination is unsupported upstream, retain explicit limits and expose a supported combination concretely rather than silently accepting lossy generation. Complete source/tests before each batch check.
3. Verify generated project builds/calls, malformed subtype behavior, source/privacy/owned API and override contracts for implemented support. Record exact library/serialization/shape evidence and remaining scope, run generation/lint/format gates, then commit/merge/push completed verified work.


Implemented slice: explicit Retrofit/Gson wrapper configuration, typed bounded options,
schema-aware unsupported-combination guard, bilingual checkbox/help, trusted fixture option
input and actual Kotlin union consumers. Reproduced and repaired Retrofit3.1 path/query
ordering through a pinned upstream template; explicit overrides retain priority.
Both dialects passed compile/call/typed roundtrip/malformed-variant checks (14 requests).
Default Moshi union support and advanced shapes/other serializers remain open in the parent plan.
