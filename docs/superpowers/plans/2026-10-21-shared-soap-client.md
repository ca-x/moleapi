# Shared SOAP client implementation plan

> **For agentic workers:** Use executing-plans inline and verification-before-completion; one independent final source review.

**Goal:** Shared editors/admins import WSDL/XSD candidates, explicitly Save references, configure/select/template/edit SOAP and invoke saved or finite draft SOAP1.1/1.2 with real XML/Fault results.
**Architecture:** Saved request/revision-bound source member routes reuse bounded core compiler/privacy and checked URL fetch. Admit SOAP in finite draft execution with actor credential/member/revision fences. Extract controlled SoapClient and SoapSourceEditor; shared hook stages definitions without implicit CAS and binds delayed callbacks to render identity.
**Tech Stack:** Existing wsdl/roxmltree/xmltree/XSD, checked reqwest, React/Radix/CodeMirror; real Spyne fixture.
**Spec:** docs/specs/soap-client.md and workspace-collaboration.md shared SOAP subsection.

## Constraints and interfaces

- Member /soap/import, /soap/import-url, /soap/schema consume request_id/expected_revision plus authored source/name or explicit URL/TLS/timeout. Schema binds saved association. No member locals or raw canonical-owner override; inherited collection/environment/request secret screening and final credential→member/revision release fence.
- URL import is explicit checked bounded HTTP, no automatic external imports or credential interpolation. Cancel/revoke/version/logout drops pending fetch. Parser/budgets remain mature existing core.
- Candidate/reference stage until explicit CAS Save; service/port/action/version/template/XML remain canonical edits. New linked definition must be saved before call. Draft execution reuses canonical workspace collection/env/schema and never writes draft to workspace; target status and structured Fault preserved.
- SoapClient consumes scope/request/specifications/schema/response/onChange/isCurrent/disabled/onImport, optional controlled confirmation. Source editor consumes guard/attach/specification and importSource callback with optional URL tuple; no owned hook in shared components.
- Every retained source/file/template/endpoint/selection/editor/confirmation callback binds mounted/current scope and render state. Viewers have no mutation/source/network/result controls. Owned behavior stays compatible.

## Tasks

1. Backend source and finite call:
   - [x] RED member source route404 and SOAP draft400; preserve raw source and reject wrong binding/viewer/foreign/stale.
   - [x] Reuse compiler/screen/checked URL helper; source watcher/final fences and SOAP saved/draft credential/version fencing; route inventory.
   - [x] Real SOAP1.1/1.2 candidate/CAS/schema/template/auth/variables/script/response/Fault/source unchanged; held URL socket revocation/version/logout cancellation and private screening.
2. Controlled UI:
   - [x] RED shared create/source stage/explicit Save/select/template/draft call/Fault flow; extract owned-compatible client and source editor, shared source hook and view controls.
   - [x] Render-bound hook/file/compiler/schema retry fences; template confirmation cannot replace changed XML or new account/request; no owner endpoints/local overrides/implicit save.
3. Verify and deliver:
   - [x] Full frontend/related backend, TypeScript/production, strict server Clippy/package format and embedded build.
   - [x] Independent review; actual English/Chinese390/1280 source import/select/template/edit/call/Fault/URL/source reopen/keyboard; screenshots/a11y evidence with installed-native boundary.
   - [ ] Coverage/map/ledger, explicit stage/commit/push, preserved PR update and exact-head CI/Docker observations.

## Review focus

- Candidate WSDL with entity-encoded canonical collection secrets must be withheld.
- Pending URL fetch must lose its socket and private result after membership/version/logout.
- Dialog or confirmation replies cannot target a newer request/draft even with the same request ID.
- Unsupported bindings/templates must show their real error without clearing original source or silently selecting a different operation.
- Finite SOAP must preserve target401 and Fault200/500, and canonical draft/source must remain unchanged by execution.

## Ledger

- Previous goal turn: progress,b90ad58 MQTT committed/pushed with713 frontend,55 related backend plus actual Broker/browser. Current worktree clean; root user mise.toml preserved.
- Existing CI39492646466 queued/check running and Docker39492646417 both images running observed; no restart. Whole28-module/395-capability objective remains active.
- Preflight: SOAP compiler/types/finite execution exist; shared draft whitelist and source routes are missing. ResponsePane is controlled and can be reused without an owned context. New source/schema association must stage before explicit Save and cannot be sent until the canonical schema exists.

- Backend: source policy RED404→GREEN; SOAP draft whitelist expanded. Exact raw bundles and explicit URL candidates do not persist until CAS; shared binding/source-private/override checks pass. Held actual HTTP sockets cover all9 source/saved/draft × role/version/logout boundaries; each result is withheld and socket closes within2s. Captured finite request headers prove canonical Bearer and pre-script header on wire, while source URL fetch sends no inherited Authorization. Real Spyne1.1/1.2 URL/CAS/schema/saved/draft/target401/Fault500/source preservation passes.
- Frontend: shared create/import stage/CAS/select/template/draft/Fault flow RED missing create→GREEN;20 related unit cases and full729/135 files pass. Owned source/hook compatibility remains. A withheld schema read left busy visible after edit/revert; regression RED→GREEN with context-valid loading projection and clean-context retry.
- Independent source review found no concrete issue; full gates/browser/native timing declined and remain verified separately. Installed-native/full platform gates remain integrated acceptance.
- Actual browser found a bootstrap issue: save rejected empty SOAP URL, preventing WSDL endpoint discovery. New actual hosted regression first failed400, then passes after core draft validation permits only empty SOAP URLs in template/save mode. Actual execution still rejects the empty URL; no invented placeholder. New5 shared SOAP tests and47 core library tests pass; related/backend Clippy/embedded gates are rerunning for this fix.

- Final core fix evidence:5 explicit shared SOAP tests and47 core tests pass; related default backend57 pass/1 configured owned SOAP fixture ignored, and owned real SOAP10 explicit pass includes both real versions. Strict core+server all-target Clippy/package fmt and embedded build pass for empty-URL fix. Full729 frontend remains green.
- Actual browser multi-file entry/bindings/XSD candidate staged at canonical revision2/spec0, explicit Save to3/spec1, CtrlEnter SOAP1.1 draft returns canonical owner variable under inherited auth, no source writes. Source reopen retains all3 original files; template Cancel preserves edited XML and explicit Replace restores it. Explicit SOAP1.2 URL candidate remains unpersisted until Save to4/spec2; original3-file source remains exact. Saved1.2 succeeds and finite draft Fault500 is structured, no canonical data changes.
- Actual narrow/a11y found Fault Detail min-content grid overflow and red text contrast4.49:1. Scoped minmax(0,1fr)/editor width and semantic red12 fix bounds and contrast in browser; production/embedded assets are being refreshed for final bilingual checks.

- Final actual embedded check after style rebuild: en/zh390 and1280 calls, success/Fault/keyboard, exact canonical4/data/source preservation; detail editor within callout; browser errors empty. Light and dark scoped a11y each0 violations/1 incomplete contrast check. Unique bilingual captures retained. Manual file authoring exercised three-file source; no native picker/installed desktop claim.
- Prior MQTT exact-head Docker39492646417 completed success for amd64/arm64 plus native container smoke; PR manifest skipped as configured. SOAP commit/push and exact-head CI/Docker observations remain next. Full395 objective is not complete.
