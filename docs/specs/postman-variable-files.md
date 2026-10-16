# Postman environment and globals files

Authority: `interchange-004` and `variables-012` in `docs/FEATURE-MATRIX.md`, the existing `WorkspaceData.environments`/`global_variables`/`Pair` models, `validate_variables`, and owner-scoped workspace CAS. This feature imports and exports Postman environment or globals JSON files into an existing workspace. It does not create team-wide variables or sync Postman's local-only current values.

## File contract

- Accept a UTF-8 Postman file with `_postman_variable_scope` exactly `environment` or `globals`, a nonempty `name`, and `values` array. An environment file maps to a new workspace environment. A globals file maps to project globals, replacing that scope only after an explicit user action. Reject unknown/missing scope, duplicate or empty keys, nonstring values, invalid flags/types, more than 1,000 entries, source above 1 MiB, or decoded values above the existing 1 MiB variable-scope cap. No partial import.
- Map `values[].key`, `value`, `enabled`, and `type` (`secret` or `default`) to a new `Pair` with new IDs and `local_value=None`. Unknown fields in individual rows may be retained only as an import warning, not silently treated as execution settings. Export produces a Postman-compatible file with the same ordered keys, enabled flags and secret markers. No request, collection, or workspace secret is emitted into an environment file unless it is a selected variable value.
- Default export clears secret variable values and screens known copied credential values using the existing `ExportPrivacy` projection. Explicit `include_secrets` exports original saved values. The file name is bounded and sanitized. Imported values remain subject to the existing workspace default/full backup behavior.

## Workspace flow

- The Environments page imports a file through a shared Rust preview API. For an environment file, the user reviews name/count and creates a new environment; an existing environment name is a visible conflict and must be renamed or canceled. For a globals file, the button says **Replace project globals** and shows the existing/imported counts before replacement. Neither action silently merges or deletes rows. The resulting change is saved with the workspace's expected revision. A failed preview or version conflict keeps the input for retry. Workspace switches invalidate the preview and require it to be rerun; account switches and logout clear the in-memory file to avoid showing one owner's variable values to another.
- Export of an environment or project globals is owned and revision-bound. If the workspace has unsaved changes, save the current draft first; reject a concurrent change rather than exporting a stale snapshot. The same server/native route performs privacy projection for both scopes.
- Standalone CLI supports importing a file into an existing owned workspace and exporting one selected scope using the shared converter and CAS route. Inputs are size-capped before persistence. No ambient Postman process or hosted third-party service is required.

## Evidence

- Pure conversion tests cover environment/globals, ordered disabled/secret rows, exact UTF-8, duplicate/missing/invalid scopes and caps, default/full privacy and source roundtrip.
- Hosted tests cover owner isolation, CAS conflicts, account/workspace switch fences, no partial import, and restored environment/globals after reload; CLI process tests use the same fixture. Inspect 390px Chinese and English import/export, keyboard and no document overflow. Run relevant Rust/frontend suites, strict Clippy, production build, branch CI and current platform gates.
