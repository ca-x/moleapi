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
