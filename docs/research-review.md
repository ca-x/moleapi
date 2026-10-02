# Research review, 2026-10-02

Status: independent artifact review complete. No critical findings; four important findings and two minor findings on the reviewed baseline. The root agent is correcting research artifacts concurrently. Findings below describe the baseline with 757 Apifox catalog entries, 1,248 Postman entries, 978 captured documents and 376 pending feature targets; they are not a verdict on later regenerated artifacts.

Scope: research notes, UI research, feature matrix, capability map, JSON catalogs and inventories, generation scripts, local official index snapshots, and selected full document bodies. Application implementation and unpublished backend drafts were excluded. Only this report was written. No network requests, implementation, commits or publication were performed by this reviewer.

## Important findings

### I1. The complete Apifox catalog drops two actual index entries

Evidence: `docs/references/apifox-llms.txt:441` and `:644` contain the links below, but neither URL exists in `docs/references/apifox-catalog.json` or the derived inventory:

- `https://docs.apifox.com/6888333m0.md`
- `https://docs.apifox.com/6888333m0.md?nav=01JHMQ9DFDQ3G5JYCPTZE4EH3J`

The FAQ title contains `dial tcp [::1]:3000`, which appears to have defeated the original Markdown link extraction. Counting the actual official-domain URLs yields 759 index entries, compared with 757 stored catalog entries. This is one unique document and its duplicate navigation entry, not two missing capabilities, but it invalidates the claim that all entries were retained and the union count of 2,005.

Correction: parse the official index without treating the first closing bracket inside a title as the link delimiter; retain both original entries and regenerate counts and all dependent artifacts. Add an index-to-catalog URL equality verification. The Postman equivalent comparison already passes with 1,248 URLs and no missing or extra entries.

### I2. Group-wide source assignment puts evidence beside capabilities it does not establish

Evidence: `scripts/build_parity_matrix.py:12` assigns the same vendor source lists to every feature in a group. Specific problematic rows include:

- `scripts-016` and `scripts-017`, generated from line 70, attach Postman Datasets pages to database pre/post processing and the combined MySQL/MongoDB/Redis/Oracle connection claim. The captured Postman pages distinguish reusable datasets and database data sources; they do not establish equivalence to Apifox's database pre/post processor. Sources: `docs/references/raw/postman-6a2dffe3b117a29f86be.json` and `postman-23b42adf7f37843e4b5a.json`.
- `administration-009`, generated from line 111, attaches Apifox organization, SSO, SCIM, audit and secret-scanner pages to BYOK. None of those five captured bodies discusses BYOK or data encryption. Postman has an already captured dedicated `/docs/administration/managing-your-team/byok-encryption` source that is not attached to the row.

The introductory disclaimer that links describe a domain reduces the risk of a literal support claim, but does not make these row-level references useful for eventual parity decisions. A consumer of `features.json` still cannot distinguish evidence for the exact capability from unrelated context.

Correction: assign evidence per capability and vendor, record whether it establishes the exact option or only contextual coverage, and retain `unknown` where evidence is insufficient. Keep MoleAPI's desired capability independent of whether either vendor supports it. The root agent has acknowledged the Datasets issue and is correcting it.

### I3. Current-product exclusions and compressed targets leave important capabilities out of the usable matrix

Evidence: `scripts/research_catalog.py:54` describes `--all` as all current product documentation, but lines 59-60 exclude the entire Postman `reference` category. The preserved index contains 11 current Newman CLI pages plus the Collection SDK page in that category. Newman custom reporters, programmatic invocation and Collection SDK interoperability are not represented as explicit targets. These remain relevant alongside the correctly documented Newman 2.1 versus Postman CLI 3.0 distinction.

Additional already captured capabilities with no explicit target include:

- Postman EU data residency and account region selection. `/docs/administration/enterprise/about-eu-data-residency`, local body `docs/references/raw/postman-5775babf857dcdf1e298.json`, also enumerates region-specific feature exclusions.
- Dataset source and query options: Postgres, SQL Server, custom JDBC, spreadsheet sources, SQL views combining sources, `pm.datasets`, SSH tunneling and the dataset YAML representation. See `/docs/tests-and-scripts/datasets/create-datasets` and `/overview`, local bodies cited in I2. `testing-015`, `testing-016`, `scripts-018` and `protocols-017` are broad labels rather than a usable enumeration of these choices.

Correction: include relevant current reference docs or explicitly record exclusions and the reason; add enumerated targets for the missing domains and dataset options. The full raw catalog is valuable evidence but does not substitute for the requested full feature-option inventory. Do not infer vendor absence from an omitted matrix row.

### I4. Vendor limits and detailed UI/configuration choices disappear between captured bodies and the feature model

Evidence: `scripts/build_parity_matrix.py:18` emits only feature name, vendor URLs, MoleAPI target, implementation status and free-form notes. `scripts/build_feature_inventory.py:29` emits documentation headings. Neither structure stores concrete option values, vendor platform, tier, version, availability or UI entry paths. Examples already available in the captured bodies:

- Apifox Vault is Commercial Flagship, with three named providers. Source: `/vault-secrets.md`, local body `docs/references/raw/apifox-5b64bb7002081be236a3.json`.
- Postman datasets are available on Solo/Team/Enterprise, live database sources require Team/Enterprise, and custom JDBC requires Enterprise. Web and desktop have different supported data-source types. Source: `/docs/tests-and-scripts/datasets/create-datasets`.
- Postman Native Git is desktop-only and distinguishes editable Local View from Cloud View and CI synchronization. Source: `/docs/use/native-git/overview`, local body `docs/references/raw/postman-da28cbc00a4db617853c.json`.

The notes are candid that detailed conditions require later module specs. Consequently, this is a credible research foundation and draft target matrix, but not yet a complete, directly implementable inventory of both products' options and UI behavior. `docs/CAPABILITY-MAP.md:3` overstates that boundary as following complete research.

Correction: preserve known vendor availability/limits and concrete choices in structured records, with exact source sections, then use that information in module specs. Describe the current artifact as a draft requirements matrix until that extraction is complete. The user's research-first requirement should remain the gate before implementation resumes.

## Minor findings

### M1. Heading extraction includes code comments and loses section hierarchy

Evidence: `scripts/research_catalog.py:43` applies `^#{1,4} (.+)$` to raw Markdown without tracking code fences. `docs/feature-options.json` consequently labels these entries from `/5801721m0.md` as `documentation_section`:

- `AF-0249-H018`: `使用 tag ` followed by the literal code name `folder` and the rest of a code comment.
- `AF-0249-H031`: `read api name from tag` followed by the literal code name `api.name`.
- `AF-0249-H084`: `parent fields first`.

These are comments inside code examples, not document sections. `scripts/build_feature_inventory.py:29` also discards heading level, parent and section locator, so repeated headings such as examples cannot be distinguished hierarchically.

Correction: parse Markdown blocks, skip fenced-code headings, retain level/parent and a stable source locator. Do not preserve 7,762 as a verified section count after fixing the parser; regenerate it.

### M2. Known Apifox Vault providers remain marked unresolved

Evidence: `docs/FEATURE-MATRIX.md:168` through `:170` mark Apifox sources for AWS Secrets Manager, Azure Key Vault and HashiCorp Vault as not located. The already captured `vault-secrets.md` explicitly names all three in the introduction and prerequisites. The matrix note asks for future provider verification even though the relevant body is present.

Correction: attach `vault-secrets.md` to those three rows and record its Flagship restriction. Leave 1Password unresolved for Apifox unless another source establishes it. Avoid attaching the same four Postman provider URLs to every provider row when an exact provider page exists.

## Verified positive evidence and limits

1. All 978 manifest `raw_file` paths existed. Every full-body SHA256 recomputation matched the manifest; no mismatches were found.
2. All 978 records were marked verified. Their full text was available locally; this review sampled capability-specific content rather than semantically reading every document.
3. All 376 MoleAPI feature entries were `pending`, and no implementation completion was asserted by these records.
4. `unmatched-matrix-sources.json` was empty. Catalog membership proves source paths are indexed, not that a source establishes every feature attached to it.
5. Postman sitemap and stored catalog URL sets were identical. Apifox mismatches are fully enumerated in I1.
6. Relative document and screenshot links in the four primary research documents resolved locally.
7. UI research explicitly discloses that no authenticated product interaction occurred, identifies Apifox screenshot age, and uses official screenshots as evidence. No application UI code was reviewed, so a code Before/After/Why table is not applicable.
8. The corrected Postman Collection 3.0 multi-file format and Newman limitation agree with the captured official collections-schema body.

Completion ledger: research artifact review done; report written; implementation not applicable; fix verification remains with the root agent after regeneration; publication not performed by this reviewer.
