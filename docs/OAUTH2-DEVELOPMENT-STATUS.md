# OAuth 2.0 development snapshot

This branch preserves ongoing OAuth 2.0 implementation. It is not part of v0.1.0 and is not a completed OAuth 2.0 release.

Implemented source includes the oauth2 5.0 SDK adapter, authorization URL and PKCE/state handling, client credentials/password/code/implicit/device grant helpers, owner/workspace token vault, refresh leases, execution integration, manual callback completion, and bilingual configuration/token dialogs.

Remaining work includes automatic hosted and desktop loopback callbacks, exposing revocation/introspection controls, broader device-flow and concurrency/privacy coverage, frontend lifecycle tests, browser QA, and independent review. Desktop opener integration still needs native platform compilation. See [the specification](specs/oauth2.md) for acceptance criteria.

Only the previously verified application revision is on main and tagged v0.1.0. This development snapshot must pass the remaining acceptance criteria before release integration.
