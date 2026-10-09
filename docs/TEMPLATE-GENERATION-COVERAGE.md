# Explicit custom template overrides

The shared OpenAPI Generator project service accepts an explicit `templates` envelope:

```json
{"format":"moleapi-codegen-templates-v1","files":[{"path":"models.mustache","content":"{{#models}}{{#model}}export interface {{classname}} { readonly templateMarker: string; }\n{{/model}}{{/models}}"}]}
```

Rendering remains OpenAPI Generator7.26.0's default mature Mustache engine. Rust uses Mustache0.9.0's existing parser to inspect partial references without calling its filesystem loader. The additive vendored entrypoint has a64-section depth cap; no application template grammar or language emitter is implemented. Only explicitly selected overrides are used; saved specifications cannot silently activate a template. Native Progenitor/Quicktype/protobuf emitters do not expose Mustache templates; choose a corresponding OpenAPI Generator target for template customization.

The UI accepts a versioned template JSON, a generated snapshot, a template directory/ZIP, or a generated project ZIP containing its preserved template source. Shared owned project import/pickers handle directory/ZIP inputs and cancellation. Template paths in directory/ZIP inputs must already be relative to the template root; a raw ZIP's arbitrary enclosing folder is not guessed or stripped. Preview/edit uses the existing editor, restore selects upstream defaults, and account/source/target changes fence late selections. The selected source is included explicitly in the generation request. After export, `moleapi-templates.json` preserves exact input source, file checksums cover it and the generation manifest records `templates_sha256`. It participates in ordinary snapshot/ZIP import and three-way regeneration as an authored artifact file.

Allow128 UTF8 .mustache files,64 KiB each,512 KiB total serialized envelope and256-character relative paths. Reject duplicate/colliding paths, traversal/absolute/backslash/URL/unsafe partial references, malformed templates and excessive sections. An application-created isolated template root owns the fixed --template-dir value. Shells/post-processing/imported Java options are not exposed; user-generated code is never executed by the generation service. Standard safe partial names can fall back to embedded upstream templates. Existing process/time/output/path/ZIP/owner/cancellation limits apply.

Default privacy checks screen templates alongside naming options against the saved specification and workspace private values, including unused source-bundle documents. Copied private values reject rather than being rewritten into different template semantics. The existing explicit include-secrets choice is required for those copies. Original specifications are not changed.

Evidence: the actual pinned Java generator rendered a custom TypeScript model override, emitted its interface marker and preserved exact source plus matching manifest/file checksums. Rust parser fixtures cover sections, changed delimiters, malicious partial paths, duplicate/traversal paths and excessive depth. The actual server route rejected private copied template content before generation and preserved canonical source. Frontend fixtures cover snapshot restore/edit, UTF8 limits, stale owner selections, explicit generation payload and removal of template previews on account changes. TypeScript, production frontend build and scoped Clippy passed.

This implements explicit override templates. Extra supporting-file definitions/output mappings, broader upstream custom-template dialect compatibility and per-framework generated-code compilation remain required work; it does not complete SDK/server/business generation or overall feature parity.
