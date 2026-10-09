# Custom templates and output mappings

The shared OpenAPI Generator project service accepts an explicit `templates` envelope:

```json
{"format":"moleapi-codegen-templates-v1","files":[{"path":"models.mustache","content":"{{#models}}{{#model}}export interface {{classname}} { readonly templateMarker: string; }\n{{/model}}{{/models}}"}]}
```

Rendering remains OpenAPI Generator7.26.0's default mature Mustache engine. Rust uses Mustache0.9.0's existing parser to inspect partial references without calling its filesystem loader. The additive vendored entrypoint has a64-section depth cap; no application template grammar or language emitter is implemented. Only explicitly selected overrides are used; saved specifications cannot silently activate a template. Native Progenitor/Quicktype/protobuf emitters do not expose Mustache templates; choose a corresponding OpenAPI Generator target for template customization.

The UI accepts a versioned template JSON, a generated snapshot, a template directory/ZIP, or a generated project ZIP containing its preserved template source. Shared owned project import/pickers handle directory/ZIP inputs and cancellation. Template paths in directory/ZIP inputs must already be relative to the template root; a raw ZIP's arbitrary enclosing folder is not guessed or stripped. Preview/edit uses the existing editor, restore selects upstream defaults, and account/source/target changes fence late selections. The selected source is included explicitly in the generation request. After export, `moleapi-templates.json` preserves exact input source, file checksums cover it and the generation manifest records `templates_sha256`. It participates in ordinary snapshot/ZIP import and three-way regeneration as an authored artifact file.

Allow128 files,64 KiB decoded bytes each,512 KiB total serialized envelope and256-character relative paths. Mustache files require UTF8 text; mapped static assets accept UTF8 or explicit Base64 content. Reject duplicate/colliding paths, traversal/absolute/backslash/URL/unsafe partial references, malformed templates and excessive sections. An application-created isolated template root owns the fixed --template-dir value. Shells/post-processing/imported Java options are not exposed; user-generated code is never executed by the generation service. Standard safe partial names can fall back to embedded upstream templates. Existing process/time/output/path/ZIP/owner/cancellation limits apply.

Default privacy checks screen templates and mapping options alongside naming options against the saved specification and workspace private values, including unused source-bundle documents. Binary static assets are screened through decoded text projections; canonical encoded bytes stay unchanged. Copied private values reject rather than being rewritten into different template semantics. The existing explicit include-secrets choice is required for those copies. Original specifications are not changed.

Evidence: the actual pinned Java generator rendered a custom TypeScript model override, emitted its interface marker and preserved exact source plus matching manifest/file checksums. Rust parser fixtures cover sections, changed delimiters, malicious partial paths, duplicate/traversal paths and excessive depth. The actual server route rejected private copied template content before generation and preserved canonical source. Frontend fixtures cover snapshot restore/edit, UTF8 limits, stale owner selections, explicit generation payload and removal of template previews on account changes. TypeScript, production frontend build and scoped Clippy passed.

## Additional generated files

The optional `outputs` object reuses OpenAPI Generator's [user-defined template configuration](https://openapi-generator.tech/docs/customization/), checked2026-10-09. All seven upstream types are available: API, APIDocs, APITests, Model, ModelDocs, ModelTests and SupportingFiles. The application passes these definitions into the mature generator's config `files` field; it does not implement per-API/model emitters or naming rules.

```json
{
  "format": "moleapi-codegen-templates-v1",
  "files": [
    {"path":"guide.mustache","content":"Guide for {{appName}}\n"},
    {"path":"AUTHORS.md","content":"Maintained by the API team.\n"},
    {"path":"asset.bin","encoding":"base64","content":"AP+A"}
  ],
  "outputs": {
    "guide.mustache":{"templateType":"SupportingFiles","folder":"extras","destinationFilename":"GUIDE.md"},
    "AUTHORS.md":{},
    "asset.bin":{"templateType":"SupportingFiles","folder":"assets","destinationFilename":"asset.bin"}
  }
}
```

API/Model-related destination names are suffixes, with one file per upstream API/model. SupportingFiles uses a complete filename and optional relative folder. Static files default to SupportingFiles and may omit the destination name to preserve their supplied relative path. Non-Mustache files copy without rendering, including binary images. Raw directory/ZIP static files get explicit default supporting mappings in the UI; generated snapshots/ZIPs retain configured mappings. The UI edits each source's role, suffix/name and supporting folder, applying changes before generation. Unmapped .mustache files retain the original override/partial behavior.

Require mappings to reference supplied source files; generated names/folders must stay relative and may not replace or nest under MoleAPI manifests. Supporting destinations cannot duplicate or collide as files/directories. Only supporting files expose an output folder; other roles use the target's package layout. Output files are not automatically made executable or executed. Preserve outputs and optional encoding fields in the same source manifest/checksum; old envelopes remain readable with omitted fields.

Actual pinned-generator evidence covers all seven roles, supporting folder/name handling, root/nested static text copy, byte-identical PNG output and source/config restoration. Server evidence also rejects private values copied into destination names and decoded binary assets. Frontend evidence covers all mapping types, confined/colliding paths, apply/error flows and binary metadata preview. Broader upstream dialect compatibility and per-framework generated-code compilation remain required; this does not complete SDK/server/business generation or overall feature parity.
