# Encoded OpenAPI source fragments

The source-bundle resolver accepts URI-fragment JSON Pointers with percent-encoded UTF8. Decode exactly once before RFC6901 pointer lookup, validate every percent triplet and tilde escape, and canonicalize equivalent encodings for model aliases and operation links. A `%25` represents a literal percent character, never a second decoding pass. Invalid UTF8, malformed escapes, missing pointers and references outside the supplied source set fail before library resolution. Literal examples/defaults/extensions are not traversed. Source files stay unchanged.

Extend the existing schematools storage/resolver rather than introducing another dereferencer. Generated internal references encode pointer characters as URI fragments; relocated operation links use the same canonical rule. Preserve recursive references, per-use annotations, discriminator aliases, owner/privacy limits and the isolated worker. Named/dynamic anchors and resource IDs remain separate required work.

Write source-bundle and owned API cases for encoded slash/Unicode/space/percent/tilde, equivalent refs, invalid escapes and source fidelity. Verify after code and tests are written, per the user's test-last preference; emitter-specific compilation evidence must remain distinct from resolver support.
