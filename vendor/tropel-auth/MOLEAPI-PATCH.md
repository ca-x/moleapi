# MoleAPI EdgeGrid compatibility patch

Source: published tropel-auth0.6.0, https://github.com/transithq/tropel, Apache-2.0. Published source/version metadata and license are preserved. Default features are disabled; only the pure EdgeGrid API is used. Other providers in this SDK are not substituted for existing dedicated MoleAPI providers.

EdgeGrid corrections verified against Postman Runtime and official Akamai signing rules:

- Only POST contributes a body hash; hash at most max_body leading bytes, including exact binary bytes. The request itself remains untruncated.
- Selected headers remain ordered, omit missing/empty fields and join with one tab; the block has no extra trailing tab.
- Use the mature URL parser instead of the upstream handwritten splitter. Actual outgoing target/query and nondefault authority port are preserved.
- Add an explicit optional signing-authority override, leaving transport URL unchanged.

HMAC/SHA256, signing-key derivation, authorization-prefix serialization and final signature remain owned by the SDK. MoleAPI supplies bounded validated credentials, deterministic test time/nonce or generated current values, and materialized request bytes/headers. Upstream tests asserting incorrect skip/PUT/trailing-tab behavior are corrected to the independent oracle; no claim is made about unused SDK providers.
