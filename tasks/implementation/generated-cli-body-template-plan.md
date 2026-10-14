# Native CLI offline JSON body templates

Parent: code-generation.md and milestone2. Keep full module scope open.

Make the existing json-body-template control execute without reading body files or making
API calls. Choose source media example, first supplied named example, schema example/default,
then upstream generated type Default if implemented. Validate candidate with generated
model deserialization and a mature JSON Schema validator against the upstream converted
schema/components before output. Preserve false/0/empty/null candidates. Missing/invalid
candidates fail with actionable guidance rather than executing the API or fabricating data.

Template flag is exclusive within the operation, exempt from required operation arguments.
Root URL/token configuration is needed only for API execution; offline templates bypass
client construction. Source example/default copies remain subject to shared privacy fences.
Do not implement a custom arbitrary-schema sampler; unsupported no-sample shapes stay open.

Write implementation/standalone consumers first: referenced/named/media/schema/default
examples, nested/enum/nullable/array models, invalid candidate constraints, no-candidate,
exclusive file/field inputs and no-network execution without URL (including invalid URL
and token). Verify roundtrip generated template as actual json-body plus owned-worker
privacy. Run full generation/API/native consumers/lint/format, document exact limits,
commit/merge/push verified work.


Implemented/verified: offline supplied/media/named/component/schema/default/model-Default
candidates, preserved zero/false/empty/null, exclusive input and client-config bypass.
DTO and mature Draft7/format validation reject malformed/no-candidate data without calls.
Added Clap string feature after reproducing enum parser compilation failure.2 template
JSON/form requests and all10 standalone fixtures (68 calls) passed,75 generation cases
and11 owned API cases passed with media-template privacy/explicit inclusion. Strict scoped
Clippy/formatting passed. Arbitrary schema sampling/selection/advanced/platform scope stays open.
