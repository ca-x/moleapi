<p align="center"><img src="assets/brand/logo.png" width="128" alt="MoleAPI Logo"></p>
<h1 align="center">MoleAPI</h1>
<p align="center">A self-hostable API workbench with an independent offline desktop client.</p>
<p align="center">English · <a href="README.zh-CN.md">简体中文</a></p>

MoleAPI uses Rust/Axum/SeaORM, React/Radix UI/CodeMirror, and Tauri. SQLite is the default; PostgreSQL and MySQL are also supported. The server embeds its frontend, fonts, and editors, so running the binary does not require Node.js. The desktop client uses local SQLite through in-process IPC and can optionally synchronize with your self-hosted server.

**Development is ongoing; complete Apifox/Postman feature coverage has not been achieved.** [Implementation status](docs/IMPLEMENTATION-STATUS.md), [protocol coverage](docs/PROTOCOL-COVERAGE.md), and [code-generation coverage](docs/SNIPPET-COVERAGE.md) distinguish verified behavior from remaining work. Dedicated clients cover HTTP, SSE, WebSocket, GraphQL, gRPC, Socket.IO, MQTT, SOAP, MCP, A2A, TCP/TLS, Data SQL, and Webhook receivers. Workspaces, collections, scoped/environment variables, scripts, history, assertions, basic Mock, and collection execution are available. Multi-language request examples and full SDK/server generation are separate capabilities; the latter remains in progress.

Collection tests support temporary or saved CSV/JSON datasets, typed iteration data, multiple iterations and cancellation. Reusable datasets use workspace save/versioning/sync; private sources are hidden in default backups. See [runner behavior](docs/DATA-RUNNER-COVERAGE.md) and [saved dataset coverage](docs/SAVED-DATASETS-COVERAGE.md).

Collection scripts can branch, loop, stop an iteration or skip a request with pm.execution controls. See [flow behavior and limits](docs/SCRIPT-FLOW-COVERAGE.md).

## Screenshots

These screenshots show the actual self-hosted interface with synthetic test data. The desktop shares this frontend; the screenshots do not establish native tray or installer verification.

![English API workbench](docs/images/workbench-en.png)

![Chinese protocol workbench](docs/images/protocol-zh-CN.png)

![Data SQL result view](docs/images/data-en.png)

![Chinese SQL editor and schema explorer](docs/images/data-zh-CN.png)

![OAuth2 authorization and private token management](docs/images/oauth2-en.png)

![Private cookies isolated by workspace and environment](docs/images/cookies-en.png)

![OAuth1 authorization and private token vault](docs/images/oauth1-en.png)

![NTLMv2 authentication against an independent HTTP server](docs/images/ntlm-en.png)

![EdgeGrid signing with independently verified binary payloads](docs/images/edgegrid-en.png)

![ASAP ES512 authentication verified by independent JOSE](docs/images/asap-en.png)

![Request network settings with a real DNS override preserving Host](docs/images/network-en.png)

![OpenAPI project generation and generated TypeScript SDK preview](docs/images/codegen-project-en.png)

OpenAPI project generation uses a native Progenitor Rust client engine and a pinned embedded OpenAPI Generator for multi-language client/server artifacts. Java targets require an explicitly configured Java17+ executable (`MOLEAPI_CODEGEN_JAVA`); native Rust does not. Original definitions are preserved, default output excludes credentials, and ZIP manifests record generated files/checksums. See [actual target validation and remaining scope](docs/PROJECT-GENERATION-COVERAGE.md).

![Generated project regeneration with directory and ZIP inputs](docs/images/codegen-regeneration-en.png)

Regeneration imports previous generated ZIPs or snapshots and current edited directories/ZIPs. It preserves authored files, compares deletions and lets you resolve conflicts before exporting a new merged ZIP. Original files are never overwritten.

OpenAPI Generator targets also accept custom Mustache templates and static assets from directories, ZIPs or preserved generation snapshots. Preview/edit templates, configure API/model/docs/tests/supporting outputs and restore upstream defaults; exports retain sources, binary assets, mappings and checksums. See [template evidence and remaining limits](docs/TEMPLATE-GENERATION-COVERAGE.md).

![OpenAPI directory/ZIP source bundle with entry selection](docs/images/codegen-source-bundle-en.png)

Single HTTP endpoints can also generate Go net/http and C# HttpClient/RestSharp request snippets, with preview, copy and source-file download. Both C# variants have real compile/call evidence; see [language coverage and requirements](docs/SNIPPET-COVERAGE.md).

Interface request-code languages and libraries follow the current Apifox/Postman lists, with23 language families and55 adapters. See the [exact language/library catalog](docs/REQUEST-CODE-PARITY.md).

Data model generation covers Apifox's19 language families using embedded Quicktype for18 languages and OpenAPI Generator for MySQL/PostgreSQL table definitions. Choose all or one component and its serialization/style options, then preview/copy/download or regenerate. See [actual model runtime evidence and remaining limits](docs/MODEL-GENERATION-COVERAGE.md).

![Data model generation and TypeScript preview](docs/images/codegen-models-en.png)

HTTP/SOAP, SSE/WebSocket/GraphQL, gRPC, TCP/TLS, MQTT, Socket.IO, A2A and MCP HTTP requests support explicit HTTP/SOCKS proxies, bypass rules, custom CA roots, PEM/encrypted PFX client certificates, DNS overrides and HTTP/2. Network credentials are scrubbed from history/default exports; exact source backup requires including sensitive values. See [network support and current combinations](docs/specs/request-network.md).

## Languages

Simplified Chinese and English UI can be selected on the sign-in screen or in workbench settings; the preference is remembered. Request names, variables, original specifications, scripts, real service responses, and generated code remain unchanged user data. Native tray menus follow the application language; native system dialogs depend on the operating system. Additional languages use embedded localization catalogs.

## Build and run the server

Building requires Rust1.94+ and Node22+. Running a built server requires only its binary.

```bash
npm --prefix web ci
npm --prefix web run build
cargo build --locked --release -p moleapi-server
./target/release/moleapi-server --bind 127.0.0.1:8787
```

Open http://127.0.0.1:8787 and create the first account using the setup token printed on first launch. Further registration requires --allow-registration. Enable --allow-private-network explicitly to debug private-network endpoints.

SQLite defaults to ./data/moleapi.db. Configure another database with --database-url or MOLEAPI_DATABASE_URL:

```bash
MOLEAPI_DATABASE_URL='postgresql://user:password@localhost:5432/moleapi' ./target/release/moleapi-server
MOLEAPI_DATABASE_URL='mysql://user:password@localhost:3306/moleapi' ./target/release/moleapi-server
```

See the [database specification](docs/specs/database.md) for configuration and backup details. Changing the URL does not migrate existing data.

## Desktop and Docker

```bash
npm --prefix web run desktop:dev
npm --prefix web run desktop:build
docker compose pull
docker compose up --no-build -d
```

Desktop builds require the platform's Tauri development dependencies. Configured installers are NSIS/MSI on Windows, DMG on macOS, and deb/rpm/AppImage on Linux. Native tray menus offer Show, Hide, Quit, and window restoration; actual support remains subject to the recorded platform checks. Signing and notarization require keys; unsigned artifacts are not advertised as signed releases.

Docker images are built, verified, and published only by GitHub Actions on native amd64/arm64 runners. Pull images locally instead of building them. The development image ghcr.io/ca-x/moleapi:feat-application may lag the latest source; Registry/Actions artifacts establish its actual version and platforms. Select a published version with MOLEAPI_IMAGE. Data uses the /data volume; external PostgreSQL/MySQL can be configured.

## Verification and automated builds

```bash
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
npm --prefix web test
npm --prefix web run typecheck
```

GitHub Actions checks source and real database cases, and builds platform server binaries, desktop installers, and Docker images. Current failed or unverified platform checks are listed in the status document. One green run does not establish full feature or installer completion.

## Feature references

- [Full capability matrix](docs/FEATURE-MATRIX.md), [module map](docs/CAPABILITY-MAP.md), [protocols and Rust libraries](docs/PROTOCOL-COVERAGE.md)
- [Apifox public catalog](docs/APIFOX-CATALOG.md), [Postman public catalog](docs/POSTMAN-CATALOG.md)
- [Configuration options](docs/CONFIGURATION-OPTIONS.md), [UI research](docs/UI-RESEARCH.md), [research notes](docs/RESEARCH-NOTES.md)

The transparent mole logo was supplied by the project owner. Competitor names and original materials belong to their respective owners and are retained as research references; the application does not use competitor brand assets.
