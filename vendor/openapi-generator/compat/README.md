# Pinned template compatibility changes

Source: unmodified OpenAPI Generator7.26.0 shaded release JAR,
`aspnetcore/3.0/Startup.mustache` (the upstream template for ASP.NET Core3.0 and later).
Apache-2.0 license: `../LICENSE`. Upstream template/header/partials remain intact.
The JAR itself remains unchanged and SHA256-pinned.

The default template emitted protected controllers without authorization middleware,
causing a real generated .NET10 route to throw500 before it reached the skeleton handler.
The compatibility copy registers authentication/authorization services and adds
`UseAuthentication`/`UseAuthorization` after `UseRouting` and before `UseEndpoints`.
An application must still configure a real authentication scheme/token validation;
no permissive scheme, embedded key or business handler is installed by MoleAPI.

The mature upstream renderer applies this template through its existing custom-template
option. User-supplied templates are written first; compatibility defaults fill only absent files.
Their original bundles/checksums remain preserved by the existing artifact path.
Generated files are never edited after rendering.

Upstream Startup template SHA256 before modification:
`1431c9ed2aeff727b9ef65a624e1bd33a8d5e1e0ac7f546b785a3b51b74472a4`.

## JavaScript binary client responses

Source: `Javascript/libraries/javascript/ApiClient.mustache` in the same7.26.0 JAR.
Apache-2.0 and upstream headers/renderer remain intact. Original template SHA256:
`7c019f32ea21f8fa6bfe14e770b4298e8253fc57c9c490411ef8774606744136`.

The API template emits the global File constructor for a binary response, while the
original ApiClient recognized only the string 'Blob'. Actual Node requests consequently
fell through text parsing and failed an exact binary-byte check. The compatibility copy
recognizes File/Blob constructors and type strings, requests SuperAgent's binary response
mode, and returns the binary body before JSON/text fallback. This uses the same mature
SuperAgent transport/parser; it does not add an HTTP/file emitter. Node returns Buffer;
the browser transport supplies Blob. Binary browser runtime remains separately to verify.

Apply only to the default `javascript` library, not `apollo`. Explicit ApiClient template
overrides retain priority. The original JAR remains unchanged and generated output is
not rewritten after rendering.

## C# generichost repeated form arrays

Source: `csharp/libraries/generichost/api.mustache` in the same7.26.0 JAR.
Apache-2.0/upstream headers and renderer are retained. A real multipart request
encoded an explicitly exploded string array as one comma-separated field.
The compatibility copy emits one field per element for required/optional arrays
whose upstream collection format is multi. Other arrays and scalars retain the
upstream serialization. FileParameter handling and the transport stay unchanged.
Apply only to the default generichost library; explicit API templates keep priority.

Original C# generichost API template SHA256:
`7daa959c929f38288f5c4c639a08060dbc395c435462fb89cb7d3950c6a96fbd`.


## JavaScript oneOf wrapper JSON binding

Source: `Javascript/partial_model_oneof.mustache`, same7.26.0 JAR; Apache-2.0
and upstream model/transport emitters remain intact. Original SHA256:
`6ae1b96a475147b7ce4ce4400762e3b2e41d474dcece7f5785e47a9a427f990f`.

A real typed union roundtrip sent SuperAgent's serialization-error string instead
of the model JSON. The wrapper's own toJSON function was copied onto the transport's
plain object, losing its prototype getActualInstance method through this binding.
The compatibility partial makes that class-field function an arrow, retaining the
original generated wrapper's lexical binding. Subtype selection/validation and JSON
serialization remain the mature upstream implementation; no custom union emitter or
transport serializer is added. Default JavaScript library only; explicit model/partial
templates keep precedence. Broader nullable/nested/ambiguous union runtime stays open.


## C# generichost selected discriminator parsing

Source: `csharp/libraries/generichost/JsonConverter.mustache`, same7.26.0 JAR.
Apache-2.0/upstream converter implementation and header are retained. Original SHA256:
`81526ce1167774baf93881155683121d3bfb146d7ae71bdd7790a6f0fc7d6261`.

A real Cat/Dog oneOf response selected/validated Cat, then failed by parsing the same
kind value using a merged parent enum that contained only Dog. For discriminator fields,
if the existing mapped-subtype pre-read produced a valid instance, the compatibility
converter skips redundant parent parsing of that field. Other fields, unknown/failed
subtype paths, constructor and serialization logic remain the upstream implementation.
This is typed discriminator decoding, not a general JSON Schema constraint validator.
Generichost only; explicit converter templates retain priority.


## Java okhttp-gson multipart collection elements/formats

Source: `Java/libraries/okhttp-gson/ApiClient.mustache` and `api.mustache`, same7.26.0
JAR; Apache-2.0/upstream headers and renderer retained. Actual wire assertions found
that the multipart loop emitted the entire list for each element, and non-multi
arrays reached that loop without their declared collection format.

The transport template now passes the current element to its existing part writer.
The API template retains multi lists and uses the upstream collection-format joining
helper for other arrays before transport. File/scalar parts and the JSON/transport
implementation remain upstream. Default okhttp-gson only; explicit templates retain
priority. This is the tested primitive string-array contract, not nested/custom part
or all-language encoding parity.

Original ApiClient.mustache SHA256: `605d3eac175edbd72231badd1dd8a1359d71a122cefe19881ec571602fe63330`.

Original api.mustache SHA256: `a041e1c4af516bdbda4a6f3b118983aa9d2950c76768128ebbb57e4666a511de`.
