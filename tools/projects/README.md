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

Default targets: `javascript,python,go,typescript-fetch,python-fastapi`; default dialects: `3.0.3,3.1.0`. `--targets` and `--versions` select subsets. `--reuse-generated` reruns selected cases from the same trusted isolated output after a driver fix, without regenerating/overwriting artifacts. Requested evidence records are cleared before the rerun, and only successful cases are written back.

Client checks cover actual GET/POST, path ID, encoded Unicode query, custom header, Bearer authentication, referenced models, enum value, nullable response, quoted Unicode JSON body and non-success HTTP errors. Go runs with `-race`. JavaScript calls use the actual generated parameter signature; Python uses named arguments because upstream3.0/3.1 parsers may order parameters differently. Both TypeScript CommonJS and ESM builds are checked by the generated package build script; the real HTTP calls use CommonJS output.

FastAPI checks package installation, process startup, generated OpenAPI routes and the explicit500 `Not implemented` response before a business subclass is supplied. No test handler is injected; successful startup does not imply authentication/business handlers exist. Client coverage does not prove complete file/binary/union/dynamic-reference support or every generator/library option.

Artifacts include generated sources/manifests, package locks or Python dependency freezes, `runtime-tools.json`, fixture-server logs for FastAPI, and `runtime-evidence.json` with engine and source hashes. Engine artifacts remain unchanged; test drivers are additional files in the isolated directories. Consumer package versions are recorded, while upstream emitted dependency ranges determine subsequent fresh installs.
