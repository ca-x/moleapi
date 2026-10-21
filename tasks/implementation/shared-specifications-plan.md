# Shared specifications plan

> Use executing-plans and verification-before-completion.

**Goal:** Shared specification browsing, OpenAPI source import/update with optional generated requests/environments, HTTP association and protected removal.
**Spec:** docs/specs/workspace-collaboration.md. Full feature matrix remains active.
**Architecture:** Member revision-fenced preview reuses OpenAPI importer, merges against canonical saved workspace and validates full candidate. Client stages returned candidate and explicitly CAS-saves. Existing sources of other kinds remain readable and support metadata/removal with reference guards.

## Constraints / review focus

- Preserve raw OpenAPI JSON/YAML, generated IDs/references and imported environments; current environment never silently switches.
- Source replacement keeps same specification ID and all existing authored requests; reject invalid linked Mock sources before Apply.
- Import/replace require clean saved version; member role/revision checks, actor-only audit, bounded source/async watcher, no owner route.
- Scope/cache/file/selection/conflict/edit changes fence late preview. No silent reference deletion; Confirm binds exact original data/ID.
- HTTP link switches clear only obsolete operation ID; named Mock links require explicit clearing before switch.

## Task 1

- [x] RED member route/UI import/replacement/reference tests.
- [x] Preview endpoint, shared panel, request association and bilingual copy.
- [x] Tests for limits/permissions/CAS/identity/references/preservation.

## Task 2

- [x] Full frontend/server, production, strict Clippy/format/embedded gates.
- [x] Independent review, bilingual390 real import/link/Mock/update/remove flows.
- [x] Coverage/docs/commit/push/PR/CI.

## Evidence ledger

Task 1: backend RED missing route404; UI RED absent specification controls. GREEN frontend636/125, targeted161 before added cache case, integration21 plus final-fence unit1. Preserve source/requests/env/IDs, source-only import, validation/dependencies, account/credential/role/revision/edit/file/cache boundaries covered.
Final review: shared_report_export_review found Important final member-check followed by awaited credential query, no second concrete issue.
Final: fixed final-await ordering — specification_preview_rechecks_after_a_pending_credential_check RED released private candidate after role change → GREEN rejects role/removal/revision changes during credential future; integration21/21.
Task 2: production45 chunks, TypeScript, strict Clippy, formatting/diff and embedded build passed. Real bilingual390 import/stage/Save/explicit environment activation/wire request/Mock/source replacement/named deletion denial/Cancel and invalid replacement retention passed, three content-free preview events, no overflow/errors.
Final: Ruling: first existing cross-instance Mock batch returned500 instead of configured401, with background database operation failures — exact isolated retry and two complete affected groups passed; retain logs and flag unknown root cause rather than modify unrelated paths without evidence — cost if wrong: an intermittent shared Mock/storage defect remains in integration and blocks a broad release-readiness claim.
Final: Ruling: headless browser acceptance pasted OpenAPI source rather than using native system file dialogs — delayed file callback/cache regression tests cover import fences; installed desktop dialog remains an open gate — cost if wrong: platform-only picker behavior can escape this pass.
