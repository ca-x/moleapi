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


## Kotlin jvm-okhttp4 non-multi form arrays

Source: `kotlin-client/libraries/jvm-okhttp/api.mustache`, same7.26.0 JAR;
Apache-2.0/upstream headers and renderer retained. Original SHA256:
`2ee424554e1953a922595e52712c599447dd2b2c8a544a641387d4fe6dff4f6d`.

Actual multipart assertions found that an explode=false array was repeated instead
of retaining its declared CSV form. The API compatibility template uses upstream
toMultiValue/collection-format metadata for non-multi non-file arrays, preserving
multi/file/scalar parts and the existing transport. Default jvm-okhttp4 only;
explicit API templates keep priority. The tested string-array contract does not
establish nested/custom-part or every encoding/platform behavior.


## Kotlin jvm-retrofit2 path parameter order

Source: `kotlin-client/libraries/jvm-retrofit2/api.mustache`, pinned7.26.0 JAR,
Apache-2.0. Original SHA256: `22c857f2d23a1b4f52665b2c196d83d4c584ffbb85415476569b228b82cc71f0`.

The3.1 fixture emitted query parameters before a path-item path parameter.
Retrofit rejects this interface at invocation: `A @Path parameter must not come after a @Query`.
The compatibility signature renders upstream path parameters first, then remaining
parameters, using Kotlin trailing commas. Upstream annotations, parameter partials,
transport, serializers and return types remain intact. Retrofit only; explicit
API templates keep precedence. Both dialect fixtures use actual typed wrappers.


## PHP Guzzle form collection formats

Source: `php/api.mustache`, pinned7.26.0 JAR (Apache-2.0/upstream header retained).
Original SHA256: `ad9429664001475e758291beb4cacbf4f879a28a64262c0f6801e72684f88b92`.

Actual multipart calls flattened primitive arrays to tags[0]/tags[1] and similarly
flattened declared CSV. For selected multipart/form-data, the compatibility API retains multi primitive lists before
the existing nested-object flattening and uses ObjectSerializer::serializeCollection
for other formats. Existing file/nested/object/transport paths stay upstream; default
Guzzle only. Explicit API templates retain priority. This is finite primitive-array
evidence, not every nested/file/encoding/library case. Raw binary SplFileObject upload
is already a correct unknown-length chunked stream; the QA server now decodes that
framing through the Python standard HTTP decoder rather than counting it as an empty body.


## PHP Guzzle mapped tagged oneOf variants

Sources: php/model_generic.mustache and php/ObjectSerializer.mustache, pinned7.26.0 JAR,
Apache-2.0/upstream notices retained. Original SHA256s: model `c8ed16cb1d244c51ac7a650bf6fcb1785e0239308ce9646d39f3f33250f3bcbc`;
serializer `42e663301eda871d3a0aff128ff92bd8eb6de18456817c7bc8bf519cc9e1eecf`.

Actual Cat wire data failed the merged Animal's Dog-only enum because the serializer
required inheritance and constructed model names from raw tags instead of using the
engine's mapped models. The model partial emits mapped oneOf classes and original discriminator wire names; the serializer
selects those ModelInterface implementations and applies their existing valid/required
checks. Missing/unknown tags reject. Other model paths retain upstream behavior; default
Guzzle only, explicit model/serializer templates retain priority. This does not provide
general JSON Schema validation or oneOf-parent PHP inheritance/static annotation parity.
