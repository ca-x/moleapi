# schematools 1.1.0 integration

Source: https://github.com/schema-craft/schema-tools, crates.io schematools 1.1.0.
License: MIT. Upstream copyright and license notices retained.

Add `Schema::from_json_at` and `SchemaStorage::from_documents` constructors.
The latter invokes upstream reference absolutization on explicitly supplied parsed
documents and never invokes the upstream filesystem/HTTP loader. Reference traversal,
cycle handling, copying and internal-reference creation remain upstream. Default
features (including HTTP/git/codegen) are disabled in MoleAPI. Any remaining semantic
or target-generator limits must be documented rather than silently ignored.

Make tera-contrib optional under codegen and enable JSON Schema TLS only under http; in-memory dereferencing does not require either dependency.

Expose upstream first-use reference locations via process_with_references for relocation of operation links, and retain permitted summary/description annotations on repeated-reference fast paths. URI resolution and reference graph traversal remain upstream.
