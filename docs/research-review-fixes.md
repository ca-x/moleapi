# Scoped research correction review, 2026-10-02

Status: reviewed corrections pass. No critical or important residual findings were identified within this follow-up scope. The artifacts support publication as a research foundation, draft capability targets and selected concrete configuration options. This is not a finding that every authenticated product control has been investigated or that any MoleAPI capability has been implemented.

The reviewed generated state contains 759 Apifox entries, 1,248 Postman entries, 1,034 verified document bodies, 8,050 documentation sections, 28 capability modules, 395 pending targets and 23 selected configuration groups. Only this report was written by the reviewer. No product code, research generator or source artifact was edited; no network requests, implementation or public actions were performed.

## Previous findings

| Finding | Result | Verification |
| --- | --- | --- |
| I1: Two Apifox index entries lost | Fixed | Both `/6888333m0.md` URLs are retained; official source and catalog URL sets now match. Total union is 2,007. |
| I2: Misleading database/BYOK source assignment | Specific examples fixed; other group references explicitly limited | Database pre/post and combined database connection rows now cite only Apifox. BYOK now cites only the dedicated Postman page. Flows has no Apifox sources. Every target has `source_mapping=domain_reference_not_feature_support_assertion`. |
| I3: Newman, data options and region capabilities omitted | Identified omissions addressed | Current reference docs are captured; matrix adds Newman CLI/custom reporters/embedded runtime/Collection SDK, a separate data module, VS Code and EU region targets. Data and reusable Dataset are explicitly distinguished. |
| I4: Missing availability and concrete option records | Materially improved and scope corrected | Source restrictions are retained as actual text excerpts. Selected controls carry literal values, documented paths when available, and platform/tier restrictions. Capability map now calls itself a draft boundary; it does not claim completed full compatibility research. |
| M1: Code comments treated as sections, hierarchy lost | Fixed | CommonMark parsing excludes the reported code comments; headings retain level, source line, parent ID and a deliberately null unverified remote anchor. Three regression tests pass. |
| M2: Known Apifox Vault providers unresolved | Fixed | AWS Secrets Manager, Azure Key Vault and HashiCorp Vault cite the captured Apifox Vault page and state the Flagship restriction. Apifox 1Password remains unresolved. |

The broader matrix remains a domain-reference requirements document, not a vendor-by-vendor support table. That distinction is now explicit in machine-readable data as well as prose. Later implementation specifications still need exact per-capability semantics and acceptance criteria; the selected-controls file does not replace that work.

## Configuration evidence review

Reviewed all 23 control records and their referenced captured bodies. No fabricated option values were identified. Most option names occur directly in their cited source. Consolidated names such as HTTP/REST, WebSocket/Socket.IO, protobuf 2/3 and the translated Collection format labels faithfully summarize the source; they should be understood as normalized research labels, not a claim that those exact combined labels appear in a dropdown.

Particularly checked:

- Apifox Body formats, authorization types, variable priority, Vault providers, scenario nodes and all 15 condition operators.
- Postman default sidebar groups and views, basic authorization types, supported protocol families and specification formats.
- Dataset data sources, spreadsheet extensions, five JDBC URL patterns, Java/driver requirements, JDBC single-source and native-SQL restriction, database/SSH fields and edition/platform limits.
- Interactive Data request source types, Desktop Agent requirements, local-file restriction and Read-only behavior.
- Enterprise EU account region preferences, their effect on new accounts and the existence of regional feature exceptions.
- Native Git's desktop-only boundary and Collection 3.0 versus Newman 2.1 compatibility.
- CLI/reporting groups are expressly documented as source section names rather than fabricated command flag values.

All control records that specify a `source_heading` resolve to a source line. Ten records intentionally cite a whole document rather than a particular heading. Remote anchors remain unverified and null, as disclosed. No authenticated UI was exercised during this follow-up.

## Verification performed in this review

1. `python3 scripts/verify_research.py` passed with 2,007 catalog entries, 275 Apifox and 759 Postman captured bodies, 8,050 sections, 28 modules and 395 targets.
2. `python3 -m unittest discover -s scripts -p 'test_*.py'` passed all three tests. They cover fenced code comments, Setext/inline heading parsing and IPv6 brackets in link labels.
3. Independently opened every one of the 1,034 manifest `raw_file` paths and recomputed the full content SHA256. Zero missing paths or hash mismatches.
4. Independently checked every retained `source_availability` excerpt against the corresponding full captured body. Zero excerpts absent from the cited body.
5. Independently checked all 395 `source_mapping` values and confirmed the three previously reported false code headings are absent.
6. Inspected the corrected Vault, database pre/post, BYOK and Flows references in generated `features.json`, not just generator source.

Completion ledger: scoped follow-up review done; report written; corrected artifact integrity verified; full authenticated UI audit outside scope and not claimed; implementation and publication not performed by this reviewer.
