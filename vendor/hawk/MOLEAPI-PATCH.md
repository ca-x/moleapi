# Hawk5.0.1 compatibility patch

Upstream: https://github.com/taskcluster/rust-hawk ; crates.io/hawk5.0.1. MPL2.0 license preserved in LICENSE; modified source ships in this public repository.

The SDK owns parsing, formatting, normalized request construction, payload hashing and cryptography. Application source only adapts actual HTTP bytes/context into its public API.

- Restore SHA1 compatibility with Postman's Hawk interface by dispatching to ring's existing SHA1/HMAC-SHA1 legacy implementations (and the existing OpenSSL equivalents).
- Add new_with_delegation without changing the existing Mac::new contract. Request header generation and verification now bind application/delegation attributes into the MAC.
- Normalize backslashes/newlines in extra data according to Hawk before MAC calculation.

Independent Postman-request fixtures and SDK tamper validation cover the changes. No application-owned cryptographic implementation or remote credential lookup is added.

- Quoted HTTP values use quoted-string0.6.1 and the pinned HTTP profile supplied by media-type-impl-utils. Escaping and unescaping remain in those mature libraries, not a new Hawk/application parser. Extra data supports escaped backslashes/quotes; timestamp parsing is integer-safe. An independent Postman-request escaped-extra-data vector covers wire encoding and MAC after decoding.
