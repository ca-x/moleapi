# Progenitor implementation patch

Source: crates.io progenitor-impl 0.15.0, https://github.com/oxidecomputer/progenitor.
Upstream license: MPL-2.0; existing file copyright notices retained.

Only the CLI emitter is changed: finite raw success responses call an asynchronous
`CliConfig::success_raw` hook, and raw error responses return the original typed
SDK error instead of an emitted panic. Raw request bodies get a --body-file
argument consumed by the upstream SDK builder. All operation discovery, arguments, request
construction and response classification remain upstream. Upgrade responses and
raw paginated responses remain unsupported and rejected by MoleAPI's emitted-code
guard. This patch is used by the generator, not required by generated projects.
