# Optional official Newman runtime

This package pins official Newman6.2.3 and its matching SDK/runtime/transformer dependencies. MoleAPI does not emulate their collection grammar, script runtime, event or reporter contracts. Native MoleAPI execution and request generation do not require this package or Node.

Install explicitly in a directory of your choice:

```sh
npm ci --ignore-scripts --prefix tools/newman
moleapi-cli newman \
  --node /absolute/path/to/node \
  --entrypoint /absolute/path/to/tools/newman/node_modules/newman/bin/newman.js \
  -- run collection.json -e environment.json -d data.csv \
  --reporters cli,json,junit \
  --reporter-json-export report.json --reporter-junit-export report.xml
```

Install third-party `newman-reporter-*` packages into the selected Newman runtime's module lookup path as documented by Newman, then pass the original `--reporters` and `--reporter-<name>-<option>` arguments. Upstream option syntax/resolution remains unchanged. The `--` separator prevents MoleAPI's own flags from consuming upstream options. Relative collection/data/report paths use the caller's working directory. No shell is invoked and nothing is installed/downloaded automatically. Unix replaces the MoleAPI process with Node, retaining exact upstream signals and exit status. Windows inherits terminal streams and returns its exit code; Windows signal/report-flush integration remains unverified.

The optional mode follows upstream report and credential behavior. Native redaction/privacy projection does not apply to Newman reports. It does not use MoleAPI server/session credentials; export an owned Postman collection explicitly when needed.

Programmatic embedding exposes the original APIs:

```js
const {newman, collection, Runtime, transformer} = require('./tools/newman');
const document = new collection.Collection({info: {name: 'Demo'}, item: []});
newman.run({collection: 'collection.json', reporters: ['cli']}, (error, summary) => {
  if (error) throw error;
  process.exitCode = summary.run.failures.length ? 1 : 0;
});
// Runtime.Runner and transformer.convert retain their official contracts.
```

必要时显式安装官方 Newman 运行时，再通过 `moleapi-cli newman` 传入原生参数。支持原始 Postman2.1 集合、环境、数据驱动、脚本及自定义 Reporter；原生 Rust 模式仍可独立使用。此模式遵循 Newman 自身的报告和凭据行为，不套用 MoleAPI 原生报告的脱敏规则。

Fixture reproduction (Node/Newman paths must be explicit):

```sh
MOLEAPI_TEST_NODE=/absolute/node \
MOLEAPI_TEST_NEWMAN=/absolute/tools/newman/node_modules/newman/bin/newman.js \
CARGO_TARGET_DIR=/absolute/cargo-target \
  cargo test -p moleapi-cli --test newman --locked -- --ignored
```

The actual fixture passed original v2.1/environment/CSV execution and pm scripts,2 requests/4 passing assertions, parsed JSON/JUnit, a custom reporter's original beforeDone/export event contract, failure exit1 and a programmatic upstream Newman run. This is a necessary runtime slice, not proof of every optional upstream protocol/auth/report/platform configuration. Docker/CI distribution and complete feature-matrix implementation remain pending. Upstream package licenses are retained in installed packages and lockfile metadata; their Apache/MIT code is not bundled into the Rust executable.
