# Native SDK builder namespace preservation

Parent: code-generation.md and milestone2.

Preserve public operation builder names while protecting standard types/constructors and
SDK support types inside the builder module. Reuse Syn's AST visitor on the mature
emitter's builder tokens: qualify standard Result/Option/String and Some/None/Ok/Err,
and parent SDK Error/ResponseValue/ByteStream/OperationInfo references. Leave operation
impl type paths (lifetime arguments), model-qualified types, identifiers/docs unchanged.
Import SDK helper traits anonymously; remove support-type name imports to avoid duplicate
bindings. No alternate operation/type emitter or source-text rewriting is introduced.

Finish implementation and standalone consumers first: public builders named Result,
Option, Some, None, Ok, Err, Error, ResponseValue, ByteStream, OperationInfo, ClientInfo,
ClientHooks, RequestBuilderExt plus actual JSON/optional/raw/pagination behavior. Extend
owned worker source/privacy artifact checks. Then run generated builds/calls and existing
fixtures, generation/API/lint/format gates, document exact evidence and merge/push.


Verified13 standard/SDK-named operation POSTs and direct public-type consumer compilation.
All12 standalone fixtures passed83 requests;78 generation cases and11 owned/privacy API
cases passed with strict scoped Clippy/formatting. Artifact presence tests accept qualified
standard Some paths while retaining their optional-body condition checks. Broader model/
hook/tag/identifier and native-platform evidence remains open in the parent scope.
