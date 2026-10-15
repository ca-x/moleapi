# Native Haskell models and total Aeson failure paths

Parent: milestone2 / MODEL-GENERATION-COVERAGE.md / code-generation.md.

Prepare explicit signature-verified isolated GHC/Aeson package paths; do not install
host packages or add interpreter execution to product. Write real compiled native
consumers before tests for original Pet/Status decode/construction/recursive/zero/null/
Unicode plus invalid enum/non-object/missing-required inputs. Aeson must return typed
Left/Nothing errors rather than throwing partial-pattern exceptions. Investigate the
actual original failure before modifying the pinned mature Haskell renderer.

Also cover multiple selected component models with distinct top-level decoders and
native alias export/array/map behavior if compiler evidence requires the seam. Reuse
mature naming, Aeson withObject/withText and existing codec instances; no custom JSON
parser/model substitute or generated-source rewrites. Preserve single-root API where
valid, user sources, enum wire strings and other language emitters. Record exact
source/license/hash/version and reproducible build patches.

After implementation/test code, run actual3.0.3/3.1.0 native cases and related profiles,
generation/model API/format/lint and prior model regressions, then integrate verified
work separately from pending Apple RPC checks. Integer/platform/advanced keyword/
union/resource/null-presence, final model/SQL/SDK/product/release scope stays open.


## Completed slice evidence

- Reproduced original partial enum/object patterns and multi-root duplicate decoder/
  alias export compiler failures; repaired only the pinned mature renderer.
- Four actual GHC9.6.7/Aeson2.2.4.0 optimized native cases pass, repeated after
  session resume in /tmp/moleapi-model-haskell-resumed-{core,multi}-{3.0,3.1}.
- All17 other emitted model targets remain byte-identical to the Crystal checkpoint;
  original Python/JavaScript/TypeScript/Go/C# native regression driver passes.
- Generation suite80 passed (43 explicit external-tool cases ignored), owned model
  API1 passed, scoped strict Clippy/format/Python/diff checks pass. Engine rebuild
  is idempotent. Exact records live in /tmp/moleapi-model-haskell-*.log and the
  isolated /tmp/moleapi-model-haskell-qa-tools metadata, not product dependencies.
- Catalog now records17 representative native model runtimes. Objective-C models,
  SQL execution, integer/platform/advanced schema and null-membership work remain
  open, along with the full parent plan. Apple protobuf run38654179920 was
  cancelled; no Apple success is claimed or merged on that evidence.
