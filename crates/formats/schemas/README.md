# Postman Collection schema

postman21.json was fetched with agent-browser from the official endpoint:
https://schema.getpostman.com/json/collection/v2.1.0/collection.json

The schema is used locally by the mature Rust jsonschema validator; import/export does not fetch schemas or execute external references at runtime. This is the Collection2.1 schema, not Collection3.0. Retained original collection source preserves unsupported config and scripts for subsequent modules.

ASAP compatibility: Postman Runtime7.56 implements the ASAP helper, while the current official2.1 auth enum still omits it (rechecked2026-10-08). The formats adapter derives a narrowly scoped local schema overlay adding only asap and its auth-attribute array; all other official rules remain unchanged. Such exports use the postman_runtime_collection.json filename, declare the helper extension in info._moleapi_runtime_auth_extensions, and return a warning code surfaced by the bilingual UI. Imports retain the original file and issue a compatibility warning. This is not a claim that an ASAP-containing file validates against the unmodified official schema.
