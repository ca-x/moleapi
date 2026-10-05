# Public test signing material

These deliberately public synthetic keys were generated with OpenSSL for JWT
sign/verify interoperability tests. They have no real account, issuer or service
identity. Never configure them as application credentials.

RSA2048, P256, P384 and Ed25519 families are tested through jsonwebtoken11.1.0.
The runtime reads explicitly supplied key content, never these fixture paths.
