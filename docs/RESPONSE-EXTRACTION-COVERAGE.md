# Response extraction coverage

Saved finite HTTP requests and collection runs support ordered declarative extraction rules. The shared Rust execution service applies them after response assertions and before post scripts. Existing requests default to an empty rule list. The React editor uses existing Radix components and bilingual labels.

| Source | Selection and result |
| --- | --- |
| JSON Pointer | Exact value via serde_json; strings remain strings, other values use compact JSON |
| JSONPath | Existing jsonpath-rust evaluator; multiple matches become a JSON array |
| XPath | Existing XPath1.0 evaluator with root namespace prefixes; external resources/DTDs disabled |
| Regex | Existing bounded regex evaluator; first capture or full match |
| Header | Case-insensitive first matching response header |
| Body | Complete UTF-8 response text |

Destinations are temporary, selected environment, collection or project scope. Environment rules require an environment selection and do not write to another environment. Successful values enter the execution scopes and ordinary returned variable updates, making them available to post scripts and subsequent requests. Browser/native clients use existing local override handling. The server does not persist extraction values into canonical workspace variables; explicit variable persistence and broader workflows remain separate requirements.

Required failures add failed checks. Optional failures add fixed warnings. Disabled rules do not execute. Missing values, malformed selectors, worker failures and exceeded limits do not silently produce replacement values. Body-based extraction rejects binary/truncated responses while header extraction remains available. Limits are100 rules,4KiB selectors,64KiB individual values and1MiB aggregate updates, within the existing bounded application child process. Returned values are privacy-tainted before application and history persistence; histories omit live updates and redact copied values. Default backups screen extraction metadata; explicit backups preserve exact rules.

## Necessary validation completed

Core selector/typed/multiple/XML/disabled/missing/value-limit cases, two real server fixtures, formats privacy/backup and assertion-worker network/timeout regressions passed. The server fixtures establish selected-environment updates visible to post scripts and the next request, unchanged canonical workspace, masked saved history, and required versus optional failures. Editor, bilingual catalogs, TypeScript, production frontend and scoped Clippy checks passed. No passing suite was repeated to establish additional claims.

Event-protocol extraction, advanced namespace declarations outside the supported root bindings, automatic shared-variable persistence, broader scenario orchestration and full product parity remain unfinished. Platform/package verification and unified review remain deferred until functional work is complete.
