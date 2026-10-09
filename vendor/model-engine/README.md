# Embedded model emitters

Pinned Quicktype core26.0.0 (Apache-2.0) and @openapi-contrib/openapi-schema-to-json-schema5.1.0 (MIT) own model emission and OpenAPI3.0 schema conversion. The browser bundle is integrity-checked during Cargo builds and embedded in both applications. No runtime Node, filesystem/network schema store, downloaded generator or hosted generation service is used.

Reproduce with `npm ci --prefix tools/models --ignore-scripts` then `node tools/models/build.mjs`. The manifest includes the exact language and renderer option catalog and bundle SHA256. Build scripts copy licenses for bundled packages and esbuild emits inline legal notices. Quicktype's npm artifact omits its license; tools/models/quicktype-core-LICENSE comes from the package's published gitHead408d4ff753d8af809a07edc40cf2a15266e01810 at https://github.com/glideapps/quicktype/blob/408d4ff753d8af809a07edc40cf2a15266e01810/LICENSE.

Run in the existing capped/cancellable project worker. The VM has128 MiB heap,1 MiB stack,2s fixed-engine initialization and5s untrusted generation deadlines; outer worker timeout/cancellation and artifact path/file/content/ZIP limits remain in force. Quicktype schema resolution has no delegated schema store. Original selected specifications and their privacy projection use the shared hosted/offline generation service.

Supported model inputs are OpenAPI components.schemas, all components or an exact selected component name. OpenAPI3.0 nullable conversion uses the mature converter;3.1 schemas pass to Quicktype's JSON Schema processor. This is not a blanket claim of support for every JSON Schema2020-12 keyword. Root/all-component generation is limited to256 top-level names. Language options are restricted to the embedded upstream catalog with typed booleans/enums/bounded naming strings. Output files, serializers and multi-file Java/Objective-C layouts remain upstream-owned.

SQL models use the separately embedded OpenAPI Generator MySQL/PostgreSQL schema targets and an explicitly configured Java17+ executable. No SQL is executed on the workspace database.
