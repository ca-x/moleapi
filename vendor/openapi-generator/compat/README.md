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
