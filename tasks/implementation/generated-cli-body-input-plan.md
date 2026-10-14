# Generated CLI typed body input exclusivity

Parent: code-generation.md and remaining milestone2.

Whole JSON-body file and generated body property flags are mutually exclusive; reject
before file read/network rather than silently overwriting field values. Distinguish body
flags from path/query/header arguments, including property-name collisions: non-body
arguments remain usable alongside the file. Sole file and sole property input retain
validation/encoding/optional-body behavior. Keep other template/interactive features open.

Add standard Clap conflicts only to generated property flags. Write standalone compile/
process fixtures for required/optional JSON and form file-only/field-only calls, conflicts
including nonexistent file, path/header property collisions, unchanged required-body
validation and no extra network calls. Extend shared owned-worker API artifact coverage.
Only after implementation/consumers are complete run runtime/generation/API/lint/format
checks, record exact evidence, commit/merge/push verified slice.


Reserved JSON control names are excluded from generated body property flags; those keys
remain supported via whole files, avoiding duplicate Clap options. Verified8 wire requests
and6 pre-read conflicts; all8 standalone runtime fixtures passed63 requests. Generation73
cases and11 owned API cases passed with strict scoped Clippy/formatting. Broader control
namespaces/template execution and native-platform requirements remain open.
