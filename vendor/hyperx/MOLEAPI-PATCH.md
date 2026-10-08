# MoleAPI compatibility patch

Upstream: hyperx1.4.0, https://github.com/dekellum/hyperx, MIT license preserved.
Source was copied from the published crates.io release. No parser, header or cryptographic code changes.

The generated Cargo.toml widens httparse1, percent-encoding2 and unicase2 dependency ranges within their existing compatible major versions. Upstream upper bounds prevent coexistence with the versions required by modern Hyper/reqwest/url. MoleAPI uses hyperx only as the www-authenticate SDK's raw header compatibility interface. www-authenticate0.4 remains responsible for token68/multiple-challenge grammar; http-auth0.1 handles bare challenge schemes.
