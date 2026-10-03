# Request execution extensions

Part of the existing full matrix; build continuously after the working foundation. Reuse mature Rust/JS libraries, retain feature modules and backwards-compatible JSON defaults.

## Variables and scripts

- Add project global variables (WorkspaceData.global_variables), module/collection variables (Collection.variables), environment variables, execution data/temporary overrides. Team globals are reserved for actual team membership module, not faked as duplicate workspace values.
- Effective precedence: temporary > execution-data > environment > collection > project; future team defaults below project. Values are typed storage strings, objects/arrays through JSON. New fields serde(default).
- Pair.local_value optional local override: native database keeps it; server workspace persistence and native cloud upload scrub it; hosted clients provide local values only in ephemeral execution overrides. No claim of native OS-Vault integration yet. Pull must preserve native local overrides by stable environment/variable identity. Existing secret markers govern redaction, not encryption claims.
- RequestSpec.pre_request_script and post_response_script default empty; optional project/module scripts can compose in documented order. JavaScript engine is QuickJS through mature rquickjs0.14.0 (Rust1.87 compatible), not a hand-written interpreter. Each phase executes in a separate worker process. QuickJS has a 64 MiB VM heap limit, 512 KiB VM stack limit and a 250 ms cooperative interrupt; native engine operations are additionally stopped by a distinct 1 second absolute host watchdog covering startup and stdin/stdout IPC. The host kills/reaps overdue workers and kills them on cancellation. There is no JavaScript filesystem/process/module network API. The in-process runtime helper alone has no hard wall-clock guarantee.
- Versioned pm compatibility for variables/environment/globals/collectionVariables, request URL/method/headers/body mutation, response json/text/status/header access, test/expect common assertions and console. Provide explicit unsupported API errors and tests rather than "fully Postman compatible". Optional pm.sendRequest must use the same bounded network policy/redirect handling, not bypass it. Collect script console/test results and field/variable changes. History must not persist local override secret values or raw tokens, including form/query-encoded variants and values created by a phase that fails. Attempted setters taint privacy independently of committing mutations; failed script details are withheld from saved history. If worker termination prevents complete taint recovery, saved response text/headers/URL/assertion details are conservatively withheld.
- /api/execute may accept variables/data overrides and returns additive logs/variable_updates. Runner chains mutations within one run, preserves assertions and counts script errors, but does not overwrite stored workspace snapshots silently.

## Richer HTTP auth/body

- HTTP API Key header/query and JWT use mature libraries; richer families get separate adapters/credentials flows, never a generic kind flag without execution.
- Multipart/binary, XML/text and GraphQL HTTP use reqwest multipart/base64 + mature parsing; upload data is explicit owned bytes, never arbitrary server file paths from an imported cURL.
- OpenAPI/source representations remain canonical; converters retain scripts/scopes and warn on truly unsupported configuration. Core tests include script loops/oversize allocation, mutable request, pre->network->post flow, variable scope precedence, local overrides not persisted/synced, and execution errors/no cross-account state.

Public paths and constructors remain compatible with docs/API-CONTRACT.md. Root owns UI/formats/workspace manifest and CI; execution implementer owns core/server and new script-runtime crate. Backend helpers isolate request engine from persistence. Root adds matching frontend types/components and import/export mappings after backend boundary is settled.


## Worker integration and limits

The server CLI and desktop application dispatch the private `--moleapi-script-worker` mode before CLI, database, network, or desktop startup. Worker input and captured private values travel through stdin/stdout pipes only; no request/credential data is placed in arguments, environment or forwarded stderr. The engine has no module loader or independent networking adapter. Four workers may run per router. Worker protocol caps are 24 MiB input and 32 MiB output, including privacy metadata; JavaScript engine input/output remain separately capped at 16/8 MiB. Native BigInt operations are supported within the worker budget rather than claimed safe through a syntax blacklist.

Library embeddings must call `moleapi_server::dispatch_script_worker()` before normal startup when using the existing current-executable constructors. An explicitly trusted absolute worker path may instead be supplied with the additive `local_with_worker(Path, Path)` or `hosted_with_worker(Config, Path)` constructors; tests use Cargo's absolute binary path. Paths are not accepted through HTTP or searched in PATH. Low-level in-process `moleapi_script_runtime::run` is for trusted tooling/cooperative tests; user scripts go through `run_worker`.


Privacy capture becoming incomplete is a phase failure even if user JavaScript catches the original quota exception. No request/variable changes from that phase are committed. History matching itself is bounded: 4 KiB per raw private value,16 KiB total raw private values,64 KiB total encoded patterns; above these limits saved text is withheld conservatively. Incomplete privacy skips matching entirely after withholding affected response data.
