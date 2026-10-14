# Native CLI named body template selection

Parent: code-generation.md and milestone2.

Extend existing offline --json-body-template to accept an optional equals-style example
name. No-value mode retains current media/first-supplied/schema/model-default behavior.
Explicit selection uses that named supplied media example (including component refs),
retains null/false/zero/empty and never silently falls back. Missing, external-only or
invalid examples reject without reads/downloads/API calls. All selected data still passes
model and mature JSON Schema validation and shared privacy screening. Preserve public
body_template default API; add body_template_named for explicit library selection.

Finish code/consumers first. Verify standalone offline default order, named/component/
Unicode/falsy values, malformed/missing/external-only rejection, exclusive input and
actual selected JSON/form request-file roundtrip. Extend owned-worker media-example privacy.
Then run full native/generation/API/lint/format gates, document exact limits and merge/push.


Runtime diagnoses: operationId string shadowed standard String, explicit named null was
collapsed by upstream Serde Option<Value>, and operationId named collided with the new
public body_template_named helper. Fixed emitted String qualification, private helper
prefixing and four-field standard Serde deserialization through versioned openapiv3 patch.
Archive hash/licenses/original fixtures retained; generated clients need no parser patch.

Verified default order, named/component/Unicode/falsy/null/default-priority, unsupported
selection rejection,2 wire requests; all11 standalone fixtures (70 calls),77 generation
cases and11 owned/privacy API cases passed. Upstream parser25 unit/fixture/doc cases and
strict scoped Clippy/formatting passed. Broad sampling/names/platform scope stays open.
