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

Default targets: `javascript,python,go,typescript-fetch,python-fastapi`; additional targets: `csharp,aspnetcore,java,spring,kotlin` (pass `--dotnet /absolute/path/to/dotnet` with a .NET10 SDK); default dialects: `3.0.3,3.1.0`. `--targets` and `--versions` select subsets. `--reuse-generated` reruns selected cases from the same trusted isolated output after a driver fix, without regenerating/overwriting artifacts. Requested evidence records are cleared before the rerun, and only successful cases are written back.

Client checks cover actual GET/POST, path ID, encoded Unicode query, custom header, Bearer authentication, referenced models, enum value, nullable response, quoted Unicode JSON body and non-success HTTP errors. Go runs with `-race`. JavaScript calls use the actual generated parameter signature; Python uses named arguments because upstream3.0/3.1 parsers may order parameters differently. Both TypeScript CommonJS and ESM builds are checked by the generated package build script; the real HTTP calls use CommonJS output.

FastAPI checks package installation, process startup, generated OpenAPI routes and the explicit500 `Not implemented` response before a business subclass is supplied. No test handler is injected; successful startup does not imply authentication/business handlers exist. Client coverage does not prove complete file/binary/union/dynamic-reference support or every generator/library option.

Artifacts include generated sources/manifests, package locks or Python dependency freezes, `runtime-tools.json`, fixture-server logs for FastAPI, and `runtime-evidence.json` with engine and source hashes. Engine artifacts remain unchanged; test drivers are additional files in the isolated directories. Consumer package versions are recorded, while upstream emitted dependency ranges determine subsequent fresh installs.

The default C# `generichost` target is built without retargeting or changing generated source. A separate console consumer configures the generated hosting extension and Bearer token provider, then checks typed nullable/enum models and HTTP400 response models. The ASP.NET Core default .NET10 project is built and started unchanged, exposing `/openapi/1/openapi.json`. Its compatibility Startup template registers and orders authentication/authorization middleware; a real authentication scheme still belongs to the consuming application. A separate test-only host configures a synthetic scheme and verifies generated-controller sample responses for authenticated requests (200) and rejection of anonymous requests (401). Default sample handlers are not production business logic. The pinned template change and license/provenance are in `vendor/openapi-generator/compat/README.md`; explicitly supplied templates override it.


## Binary upload/download profile

Select clients and add `--profile binary`:

```sh
python3 tools/projects/verify-runtime.py   --worker /absolute/path/to/target/debug/moleapi-server   --java /absolute/path/to/java   --dotnet /absolute/path/to/dotnet   --targets javascript,python,go,typescript-fetch,csharp   --profile binary   --output /absolute/path/to/new-binary-fixture-directory
```

The core profile remains the default. Binary cases retain all core calls and add authenticated octet-stream download/upload. Raw bytes `00 01 02 0d 0a ff 80 41` must survive exactly, including invalid UTF8 and NUL. The fixture verifies request Content-Type/raw bytes and returns an integer receipt. Consumers use upstream Buffer/Blob, file, bytes or Stream/FileParameter APIs; no generated source is modified after rendering. Each client/dialect case performs five real requests. Runtime evidence now includes the selected profile.

The default JavaScript library uses the documented pinned ApiClient compatibility template to recognize File/Blob binary response types and retain the mature transport's binary body before text/JSON fallback. Explicit templates override it; Apollo is untouched. The verified runtime is Node; browser binary execution remains a separate gate. This finite payload case does not establish multipart, huge-file streaming, nonseekable uploads, resume/range or binary-error-body coverage. Some upstream clients buffer responses or create temporary download files; the test consumers close/delete those resources.


## Multipart file upload profile

Use the same client selection and explicit tools with `--profile multipart`. The server parses MIME using Python's standard email/MIME parser and checks exact non-UTF8 file bytes, filename `payload.bin`, file-part `application/octet-stream`, quoted Unicode note, repeated required and optional tags, and a deliberately comma-packed `explode=false` array. Each client/dialect case retains the three core requests and performs one multipart upload. All five default clients passed both3.0/3.1 variants.

A pinned C# generichost API template fixes the observed conversion of an exploded array into a single CSV field. The existing renderer emits one field per element for upstream multi-format arrays and preserves scalar/non-exploded serialization. Other C# libraries and explicit API templates are not replaced. The fixture uses one finite file and known MIME; multiple files, custom part types/headers, nested objects, arbitrary delimiters, huge/nonseekable streams and browser execution remain separate coverage gates.


## Referenced tagged-union profile

Use the same five default client targets with `--profile union`. The3.0/3.1 fixture declares referenced Cat/Dog oneOf members with distinct required fields, tag enums and explicit discriminator mappings. Each client performs the core three requests, GETs both variants and POSTs the decoded generated model instances back; the fixture checks exact JSON and fields. JavaScript uses the generated getActualInstance wrapper, Python its actual_instance, Go/C# their union member properties, and TypeScript's decoded tagged model. Each case makes seven real requests. C# additionally rejects missing/unknown tags and missing selected-subtype required fields using the generated converter.

Pinned default-library partials fix two reproduced defects: JavaScript wrapper toJSON lost this binding when copied by SuperAgent, and C# revalidated a known selected subtype with an incorrectly merged parent enum/required check. The existing model/transport/converter implementations still perform selection, validation and serialization. Explicit oneOf/converter templates retain priority. This case establishes two object variants with simple tags; nested/nullable/ambiguous unions, anyOf, arbitrary root constraints and other languages/browser runtimes remain separate gates.


## Default Java client and Spring skeleton

Use `--targets java,spring --profile core --jdk /absolute/path/to/jdk --maven /absolute/path/to/mvn`, retaining the required explicit application worker and generator Java executable. The JDK must include javac; Maven/JDK are consumer QA prerequisites, not extra product runtime dependencies. Maven uses the emitted POM and its ordinary dependency cache; the harness sets JAVA_HOME for only these build processes. Fresh builds require registry access/cache. Tool versions and generated POM/classpath files remain in the isolated output.

Java uses the default okhttp-gson library, creates a generated ApiClient/DefaultApi/Pet consumer, builds the emitted project with Maven and calls the real core fixture. Generated method parameter order is read from the actual signature for each dialect. Generated placeholder tests are skipped; the explicit consumer performs the real assertions. Spring uses the default spring-boot library, packages its emitted executable, starts it on loopback and checks `/v3/api-docs`, generated routes and the declared501 business-handler placeholder. It does not inject a handler/security implementation. Java binary/multipart/union consumers are available through the same profiles; Spring extended consumers remain separate gates.


Default Java okhttp-gson now also accepts `--profile binary`, `multipart` and `union`. Binary consumers use returned File objects and compare/delete temporary downloads; uploads use actual File inputs. Multipart sends one named binary file, quoted Unicode note, required/optional repeated tags and non-exploded CSV tags. Union consumers use generated getActualInstance wrappers and POST the decoded model instances without custom serialization. All6 extended profile/dialect cases passed. The API's escaped `_file` parameter and any dialect-specific argument order come from its actual signature.

Pinned default-library ApiClient/API templates fix two observed multipart failures: the transport loop now writes each current list element, and non-multi arrays use the engine's existing collection-format join helper before transport. Explicit templates keep precedence; other Java libraries are untouched. Larger/nested/custom/delimiter/nonseekable/resume/browser and advanced union/dynamic semantics remain separate gates.


## Real browser SDK profiles

`verify-browser.py` runs fresh default JavaScript and TypeScript-fetch clients in an isolated agent-browser Chromium session, using the same exact loopback wire assertions. Install agent-browser/Chrome and provide the repository's esbuild executable (the frontend development dependency):

```sh
python3 tools/projects/verify-browser.py   --worker /absolute/path/to/target/debug/moleapi-server   --java /absolute/path/to/java   --output /absolute/path/to/new-browser-fixture-directory
```

Defaults are two clients, two dialects and `core,binary,multipart,union`:16 combinations/76 actual API calls. `--targets`, `--versions`, `--profiles`, `--browser`, `--npm`, `--esbuild` and `--reuse-generated` select explicit tools/subsets or rerun the same trusted fixtures. Standard browser bundling uses generated source/package browser entries; Node transport, mock SDKs and custom serialization are not substituted. Browser-native File/Blob payloads retain bytes/name/MIME and form-array/union behavior. The page and API share one loopback origin; cookies/cross-origin/CORS and real desktop WebViews remain separate gates.

The harness derives and closes its own named browser session, shuts down its owned server, and preserves package locks, generated projects, entrypoints/bundles, browser/tool versions, engine/source/bundle hashes and successful profile records. Test pages contain only trusted synthetic data. Preview/download does not launch a browser or execute imported projects.


## Default Kotlin generated Gradle project

Use `--targets kotlin --profile core --jdk /absolute/path/to/jdk --gradle /absolute/path/to/gradle`. The verified Gradle8.14.5 matches the emitted wrapper version; the library stays `jvm-okhttp4`. The emitted project uses Kotlin plugin2.4.0, Moshi1.15.2 and OkHttp5.4.0. An added trusted Kotlin consumer and separate Gradle init task compile the original build and execute actual core wire/model/error assertions. The generated build/source is not retargeted or rewritten. Generated placeholder tests are skipped; the explicit consumer performs the real checks. JDK/Gradle/registry cache are QA prerequisites and add no runtime dependency/download to preview.

Both3.0/3.1 cases passed using Gradle/JDK21 on Linux. The consumer bounds timeouts and closes its OkHttp connection/executor resources. Kotlin file profiles are available below; union, other libraries/platforms and broader keyword behavior remain separate gates.


Default Kotlin `jvm-okhttp4` also supports the trusted `binary` and `multipart` QA profiles. Consumers use its actual File APIs, named parameters and escaped `propertySize` receipt field; generated Gradle/source stays unchanged. All4 profile/dialect cases passed. Raw bytes, file name/MIME, Unicode text, required/optional repeated arrays and non-multi CSV use the same exact assertions as other clients. A pinned API compatibility template reuses upstream toMultiValue for non-multi non-file arrays instead of repeating them; explicit API overrides and other libraries retain their behavior. Kotlin union and broader file/platform scenarios remain separate gates.


## Explicit Kotlin tagged union configuration

For the supported upstream wrapper combination, generate fresh fixtures with:

```sh
python3 tools/projects/verify-runtime.py --worker /absolute/path/to/moleapi-server --java /absolute/path/to/java --output /absolute/path/to/new-fixtures --targets kotlin --profile union --generator-options '{"library":"jvm-retrofit2","serializationLibrary":"gson","generateOneOfAnyOfWrappers":true}' --jdk /absolute/path/to/jdk --gradle /absolute/path/to/gradle
```

Both3.0/3.1 cases compile the original Gradle build and use generated Retrofit services,
Animal wrappers and registered Gson adapters. Core wire behavior plus Cat/Dog decode/echo
and malformed variant rejection passed;14 actual requests. A library-specific compatibility
API template orders path parameters before query parameters. Explicit templates keep priority.
The fixture example accepts the same JSON options as its final positional argument;
evidence records capture actual manifest options, including when reusing generated output.
Default Moshi unions reject explicitly. For `kotlinx_serialization`, replace the serializer value in the JSON option object.
The driver selects actual generated sealed-wrapper and kotlinx serializer APIs from the
manifest. Both dialect cases passed the same14 requests and malformed-variant checks;
the emitted project uses kotlinx-serialization-json1.11.0. Both Gson regression cases
also passed. Broader union shapes/default-Moshi/platform scenarios remain open.


## Default PHP Guzzle SDK

Provide explicit PHP8.1+ with curl/json/mbstring and an absolute Composer PHP entrypoint
(executable PHAR/script); the driver invokes it through that PHP interpreter:

```sh
python3 tools/projects/verify-runtime.py --worker /absolute/path/to/moleapi-server --java /absolute/path/to/java --output /absolute/path/to/new-php-fixtures --targets php --profile core --php /absolute/path/to/php --composer /absolute/path/to/composer.phar
```

The unchanged default composer manifest installs with --no-dev/--no-plugins/--no-scripts.
The driver records composer.lock/dependency JSON/tool metadata, lints generated lib PHP,
and calls actual DefaultApi/Pet/Failure classes. Named PHP parameters accommodate the
engine's3.0/3.1 parameter order; typed400, enum/null, Bearer and exact Unicode wire fields
are asserted. Both dialect cases passed6 requests using PHP8.5.11/Composer2.10.3 and
Guzzle7.15.5 on Linux. Explicit wrapper paths may configure isolated PHP modules/libraries;
QA package preparation does not modify application/runtime behavior. Reuse generated
fixtures with --reuse-generated. Binary/multipart/union and other PHP options/platforms
require their own consumers and remain unverified by this core profile.


Default PHP also supports the trusted binary and multipart QA profiles with the same
explicit --php/--composer paths. Four3.0/3.1 cases passed18 requests. Binary upload uses
SplFileObject's actual chunked stream; the shared server decodes standard chunked framing
and compares exact bytes. Multipart uses SplFileObject plus required/optional repeated
arrays and declared CSV. A pinned Guzzle API template repairs primitive-array flattening;
explicit templates keep priority. Generated PHP and composer manifests remain upstream
except that documented compatibility default. Run focused framing regression with:

```sh
python3 tools/projects/test_verify_runtime.py
```

This checks binary framing/extensions/trailers/stream ownership and incomplete-body
rejection. PHP unions, custom/nested/multiple files, streaming size/platform/other-library
contracts remain separate profiles, not implied by these finite file consumers.


## Default PHP mapped tagged unions

Use --targets php --profile union with the same explicit PHP/Composer paths. Default
referenced Cat/Dog mappings now decode actual generated variants and echo model objects.
The pinned default-Guzzle model/serializer templates retain upstream mappings, original
wire field names and existing required-field validation. Missing/unknown tags and missing
subtype fields reject. Explicit templates retain priority.

Optional --discriminator-property animal-kind (or 0) exercises PHP normalized/falsy wire
names; the fixture example accepts the same wire name after its JSON options argument.
These alternate fields currently have PHP consumers only. Three field names × two dialects
passed42 union requests; unchanged core/binary/multipart passed24 regression requests.
Reuse runs must select the same wire name as the generated fixture.

The models remain ModelInterface implementations; merged Animal/static parent PHPDoc and
inheritance are separate incomplete contracts. AnyOf/untagged/advanced shapes, strict
primitive/schema validation, other libraries/platforms require further evidence.
