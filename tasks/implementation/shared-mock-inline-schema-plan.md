# Shared inline Mock Schema editing

Authority: `docs/specs/workspace-collaboration.md`, shared Mock editor parity.

- Extract the owned Schema JSON dialog into a pure reusable component, preserving the 256 KiB JSON-object edit gate and server validation on workspace Save.
- Add a shared source selector for static body and inline Schema. Existing specification links or data bindings remain selected and preserved until the member explicitly switches source; switching to static/inline clears incompatible references in the draft. Preserve static body, scripts, traffic settings and headers.
- Offer the existing field-name inference toggle for inline or already linked Schemas. Do not invoke owner-only schema validation, publication or data-source management routes.
- Key shared Schema dialog lifetime by account/credential/owner/workspace/role, saved revision, request and example. A remote revision or selection change closes the old dialog before its local text can be committed elsewhere.
- Tests first fail for absent shared Schema controls; cover invalid JSON, canonical CAS save, preserved static body/reference switching and remote-dialog invalidation. Retain owned Schema regressions, full frontend/catalog/type/build checks, real bilingual 390 px generated response and independent review. Broader link selection, script/data/matching editors and current-head remote/native gates remain open.

## Execution record

- Shared editing and remote-dialog tests failed before the controls existed, then passed. Source preservation and explicit replacement tests cover both existing links and data bindings; remote invalidation covers clean and dirty-conflict workspaces.
- Owned/shared focused 64/64 and full frontend 496/496 passed, including catalogs, with TypeScript, 45-chunk production build and rebuilt embedded server. Independent source review found no concrete privacy, role, stale-dialog or compatibility issue.
- A real 390 px browser blocked invalid JSON, saved valid Schema and inference via member CAS, previewed id 7 with generated email, then switched to static and restored the original body. Owner API confirmed the source fields and preservation. English and Chinese had no overflow or browser errors. Remote CI/native gates remain pending.
