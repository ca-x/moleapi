# Classic Dubbo provider discovery and metadata import

This extends `dubbo-client.md` and matrix row `protocols-015` beyond direct calls. Apifox documents ZooKeeper, Nacos and Alibaba Cloud EDAS registry selection, plus provider metadata-center import. Registry discovery and metadata import are distinct: the first finds a provider for one call, while the second reads service/method/parameter definitions into an editable source bundle.

## Source and selection

- A versioned Dubbo request may select `direct` or `registry`. Registry source names the provider (`zookeeper`, `nacos`, `edas`), registry endpoint/region/namespace, target service/version/group and explicit credentials. An optional metadata-center source is separate. Direct requests retain the existing exact source and defaults.
- Save the original registry and metadata references, even when a current instance cannot connect. No URL, service, method or argument is rewritten into the canonical workspace after discovery. Explicit source export can preserve credentials; default backup, history and reports screen them. External Postman/OpenAPI exports refuse unsupported Dubbo source.
- Discovery returns a bounded set of provider URLs and provenance, not an already opened provider socket. The user can inspect which provider was selected. Registry and metadata responses are treated as untrusted remote data.

## Checked two-phase call

1. Resolve every registry/metadata destination address under the existing owner and network policy. Pass only a checked literal IP to an isolated mature SDK worker, preserving the original logical name only where the SDK can safely use it for TLS/SNI/authentication. Unsupported TLS/proxy/DNS settings reject explicitly.
2. Ask Apache Dubbo's registry API for a one-shot provider snapshot matching service/version/group and provider category. The worker must not register this client as a provider, start QoS, expose a listener, execute provider RPCs or cache credentials to disk. Bound startup, response bytes, provider count and URL size; cancellation kills and reaps it.
3. Rust validates every returned provider URL's scheme, service/version/group/serialization and all resolved IPs. It rejects mixed safe/unsafe address sets under hosted policy. It deterministically chooses one validated provider and passes its pinned IP to the existing direct GenericService worker. No Java re-resolution of provider names.
4. Discovery and invocation have one user-visible deadline. On failure, report whether registry lookup, provider admission, connection or remote method failed. Do not replay an ambiguous method result or silently switch to another provider after a request has begun.

Registries are eventually consistent. A provider can disappear after lookup, so a validated snapshot is an address selection, not a guarantee of availability or a transaction. No persisted or cross-owner provider cache is required for the first slice; each explicit call rechecks registry and selected provider. A later bounded cache must state its staleness and revision rules.

## Metadata import

Use Apache Dubbo's provider metadata service/metadata report APIs to read provider-advertised package, interface, method, parameter and response definitions. Import is explicit, owner-scoped, capped and cancellable. Preserve the original metadata document/provenance and present a preview/diff before applying it to a workspace; never overwrite authored definitions without a selected conflict decision. Dubbo providers without metadata-center reports, or Dubbo versions/options the engine cannot decode, return an explicit unsupported result. Metadata endpoints follow the same checked-address and credential rules as registries.

## Acceptance evidence

- Separate real ZooKeeper and Nacos provider fixtures cover lookup, version/group filtering, provider removal/change between calls, registry auth, malformed/oversized URLs, empty results, cancellation and private-network refusal. Independent Java services answer the selected direct RPC, proving the provider endpoint came from registry output and was rechecked/pinned by Rust.
- EDAS uses an explicit test account/sandbox or equivalent provider-approved fixture; API name, region/namespace, credential scope and endpoint behavior are verified separately. Compile-only SDK use or a generic registry selector does not count.
- Metadata import demonstrates original source and method/DTO mappings, owner isolation, private export, preview/conflicts and exact restore. Missing/unsupported metadata remains visible without fabricating a definition.
- Bilingual editor and actual browser/native flows show registry/source selection, selected provider, errors and cancellation at390px. Three databases, installed packages, embedded resources and container smoke remain the unified release gate.

Source: retained hash-checked Apifox `create-dubbo-api.md` and `debug-dubbo-api.md` under `docs/references/raw/`, plus Apache Dubbo3.3.6 `RegistryService.lookup(URL)`, `RegistryFactory` and metadata-report source. The direct worker is not a registry client merely because it accepts a `dubbo://` URL.
