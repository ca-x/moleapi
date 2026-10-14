# Generated SDK/server runtime verification

Run only with trusted synthetic fixtures. This tool is independent of product preview/download and never runs imported projects. It generates projects through `ProjectRuntime` and the pinned embedded OpenAPI Generator7.26.0, installs their consumer dependencies into isolated projects, compiles where applicable, and calls a controlled loopback fixture.

Prepare the application executable and pinned engine, then run:

```sh
python scripts/prepare_openapi_generator.py
cargo build -p moleapi-server
python3 tools/projects/verify-runtime.py \
  --worker /absolute/path/to/target/debug/moleapi-server \
  --java /absolute/path/to/java \
  --output /absolute/path/to/new-fixture-directory
```

The output must be a new directory. Java17+, Cargo, Python with venv/pip, Node/npm, Go with a race-capable C toolchain, and consumer dependency registry access/cache are prerequisites. `--node`, `--npm`, and `--go` can select explicit executables. The application itself does not acquire these extra runtimes. Generated dependency installation/build scripts run only in this explicit QA command.

Default targets: `javascript,python,go,typescript-fetch,python-fastapi`; additional targets: `csharp,aspnetcore` (pass `--dotnet /absolute/path/to/dotnet` with a .NET10 SDK); default dialects: `3.0.3,3.1.0`. `--targets` and `--versions` select subsets. `--reuse-generated` reruns selected cases from the same trusted isolated output after a driver fix, without regenerating/overwriting artifacts. Requested evidence records are cleared before the rerun, and only successful cases are written back.

Client checks cover actual GET/POST, path ID, encoded Unicode query, custom header, Bearer authentication, referenced models, enum value, nullable response, quoted Unicode JSON body and non-success HTTP errors. Go runs with `-race`. JavaScript calls use the actual generated parameter signature; Python uses named arguments because upstream3.0/3.1 parsers may order parameters differently. Both TypeScript CommonJS and ESM builds are checked by the generated package build script; the real HTTP calls use CommonJS output.

FastAPI checks package installation, process startup, generated OpenAPI routes and the explicit500 `Not implemented` response before a business subclass is supplied. No test handler is injected; successful startup does not imply authentication/business handlers exist. Client coverage does not prove complete file/binary/union/dynamic-reference support or every generator/library option.

Artifacts include generated sources/manifests, package locks or Python dependency freezes, `runtime-tools.json`, fixture-server logs for FastAPI, and `runtime-evidence.json` with engine and source hashes. Engine artifacts remain unchanged; test drivers are additional files in the isolated directories. Consumer package versions are recorded, while upstream emitted dependency ranges determine subsequent fresh installs.

The default C# `generichost` target is built without retargeting or changing generated source. A separate console consumer configures the generated hosting extension and Bearer token provider, then checks typed nullable/enum models and HTTP400 response models. The ASP.NET Core default .NET10 project is built and started unchanged, exposing `/openapi/1/openapi.json`. Its compatibility Startup template registers and orders authentication/authorization middleware; a real authentication scheme still belongs to the consuming application. A separate test-only host configures a synthetic scheme and verifies generated-controller sample responses for authenticated requests (200) and rejection of anonymous requests (401). Default sample handlers are not production business logic. The pinned template change and license/provenance are in `vendor/openapi-generator/compat/README.md`; explicitly supplied templates override it.
