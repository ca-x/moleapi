# Generated CLI API/control argument namespace

Parent: code-generation.md and milestone2.

Keep whole-body controls and automatic help distinct from legitimate API arguments.
Reserve json-body/json-body-template/body-file/help in the operation parser. Colliding
API arguments use deterministic api- prefixes, repeated until free against all original
API names/control names; SDK setters and wire keys remain original. Help states the
original API parameter. Body help/control-key properties retain full-file fallback.
Disable Clap's auto help subcommand at the executable root so operationId=help can run;
--help still works. Keep template execution and other discovery workflows open.

Write implementation and standalone compile/process fixtures first: JSON and binary
controls alongside reserved API query/header/path names, preexisting api- aliases,
operation help, actual wire/body/Bearer preservation and no-network --help/missing-control
behavior. Extend owned-worker artifact coverage. Then run existing generated programs,
generation/API/lint/format gates, document exact evidence, commit/merge/push verified work.


Verified3 actual requests and no-network help/missing-body checks. All9 standalone
fixtures passed66 requests;74 generation cases and11 owned API cases passed with strict
scoped Clippy/formatting. Broader namespaces, template execution and native-platform
requirements remain open in parent coverage.
