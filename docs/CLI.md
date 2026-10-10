# MoleAPI CLI / 命令行

The native CLI reuses the server's execution, format conversion and request-code engines. Offline commands use an in-process router and do not start background schedule or notification delivery loops. A serving instance delivers queued notifications. No Node runtime is required for request snippets.

命令行复用服务端的执行、格式转换和请求代码生成模块。离线命令不会启动后台定时任务或通知投递循环；已排队的通知由运行中的服务端投递。请求代码生成不需要安装 Node。

```sh
# List all23 request-code language families and55 library variants.
# 列出完整语言和客户端库目录。
moleapi-cli snippet

# Import once into a persistent local database.
moleapi-cli --database ./moleapi.db import --input ./collection.json --format postman --name Demo

# Use workspace/request IDs or unique names. Select a library from the catalog.
moleapi-cli --database ./moleapi.db snippet --workspace Demo --request Login --target go --client native --output login.go
moleapi-cli --database ./moleapi.db snippet --workspace Demo --request Login --target csharp --client httpclient --output login.cs
moleapi-cli --database ./moleapi.db snippet --workspace Demo --request Login --target java --client okhttp --output Login.java
moleapi-cli --database ./moleapi.db snippet --workspace Demo --request Login --target python --client requests --output login.py

# Select requests within a collection subtree; repeat --request as needed.
moleapi-cli --database ./moleapi.db run --workspace Demo --collection Smoke --request Login --request Profile

# Run an imported collection in an isolated temporary database; export before cleanup.
moleapi-cli run --input ./collection.json --input-format postman --collection Demo --environment Staging --ci --no-notifications --report ./report.xml --reporter junit

# Remote service authentication: credentials come from environment variables.
moleapi-cli --server https://api.example.com login --username tester --password-env MOLEAPI_PASSWORD --output ./token.txt
moleapi-cli --server https://api.example.com --token-file ./token.txt run --workspace Demo --collection Smoke --ci --report ./report.html --reporter html --language zh-CN

# Command discovery for tooling.
moleapi-cli schema
```

Default snippet/export output screens saved private values. Use `--include-secrets` explicitly when the generated example requires them. Output files are atomic and private; `--overwrite` is required to replace existing files. Raw file bodies expose only capable adapters. See [the complete language/library list](REQUEST-CODE-PARITY.md) and [generation limits](SNIPPET-COVERAGE.md).

默认代码和工作区导出会隐藏已保存的敏感值，需要原值时显式指定 `--include-secrets`。文件采用私有权限和原子写入，覆盖需加 `--overwrite`。原始文件请求体只提供具备文件读取能力的库选项。语言清单与请求体限制见上述链接。

Runs return0 for passing nonempty execution,1 for failed/empty/stopped execution,2 for usage/transport/storage/export errors and130 after Ctrl-C cancellation. `--input` workspaces are temporary: stdout identifies them as such and omits a persistent report ID. Export with `--report` to retain results. A persistent `import` followed by `run --workspace` retains owned reports.

Private run variables use `--variables-file PATH` or `--variables-env NAME` (exclusive). The environment option names an OS variable containing JSON. Keep actual values in a private file or CI secret store, outside command arguments and repository collections:

```json
{
  "temporary": {"access_token": "runtime-secret"},
  "project": {"obsolete": null},
  "collection": {"tenant": "runtime-tenant"},
  "environment": {"api_key": "runtime-key"}
}
```

```sh
moleapi-cli --database ./moleapi.db run --workspace Demo --collection Smoke --environment Staging --variables-file ./private-variables.json
moleapi-cli --server https://api.example.com --token-env MOLEAPI_TOKEN run --workspace Demo --collection Smoke --environment Staging --variables-env MOLEAPI_RUN_VARIABLES
```

Inputs are bounded to1MiB. `temporary` accepts strings; the other scopes accept strings or `null` deletions. Unknown envelope fields are rejected before import/execution. Overrides reuse existing scope precedence (temporary overrides environment, collection and project), remain private in native reports/history and do not save changes to workspace variables. Environment overrides require a selected or active saved environment. Use temporary variables when no profile exists. Native JSON overrides and original Newman environment files have separate formats.

运行变量支持互斥的 `--variables-file` 和 `--variables-env`，后者读取指定系统环境变量中的 JSON。临时作用域接受字符串，项目、集合和环境作用域还支持 `null` 删除；临时值优先于环境、集合和项目值。输入最多1MiB，会在导入和执行前校验。所有注入值按私密变量处理，不写回已保存的工作区；环境覆盖需要选中或已激活的环境配置。服务端访问令牌与接口请求中的令牌分别配置。

GitHub Actions can invoke an installed CLI without Docker:

```yaml
- name: Run API smoke tests
  env:
    MOLEAPI_TOKEN: ${{ secrets.MOLEAPI_TOKEN }}
    MOLEAPI_RUN_VARIABLES: ${{ secrets.API_RUN_VARIABLES_JSON }}
  run: >-
    moleapi-cli --server https://api.example.com --token-env MOLEAPI_TOKEN
    run --workspace Demo --collection Smoke --ci --no-notifications
    --variables-env MOLEAPI_RUN_VARIABLES
    --report report.xml --reporter junit
- uses: actions/upload-artifact@v4
  if: always()
  with:
    name: api-report
    path: report.xml
```

GitLab CI can run the same command and attach `report.xml` under `artifacts:reports:junit`. Jenkins can run it in a shell stage and publish `junit 'report.xml'` in an always/post step. Supply credentials through the CI secret store. CLI installation/packaging is deferred until functionality is complete. These commands do not claim complete Newman compatibility, external custom reporters or full matrix completion.

An optional original Newman CLI/runtime now uses explicit installed Node/Newman paths, forwarding upstream arguments and custom reporter behavior. Programmatic original SDK/runtime/transformer APIs are exposed too. See [setup and actual fixture evidence](../tools/newman/README.md). This requires that optional runtime; native commands remain independent of it. Newman reports follow upstream credential behavior rather than native redacted-report rules. Docker/platform/package integration remains pending.

Personal access tokens are independent of login sessions. Manage them with a login-session credential and use them for existing remote commands:

```sh
moleapi-cli --server https://api.example.com --token-file ./session.txt tokens create --name CI --days 90 --output ./ci-token.txt
moleapi-cli --server https://api.example.com --token-file ./session.txt tokens list
moleapi-cli --server https://api.example.com --token-file ./ci-token.txt list workspaces
moleapi-cli --server https://api.example.com --token-file ./session.txt tokens revoke --id TOKEN_ID
```

Created plaintext is written only to the private output file; stdout is metadata. An API token cannot manage other tokens. Revocation invalidates future token requests, stops this account's active tasks/connections on the current instance and propagates to other hosted instances sharing its database. See [exact behavior and evidence](ACCESS-TOKEN-COVERAGE.md).

Agent tooling can use the embedded standard skill without a database or service connection:

```sh
moleapi-cli skill
moleapi-cli skill --format json --output moleapi-skill.json
mkdir -p .agents/skills/moleapi
moleapi-cli skill --output .agents/skills/moleapi/SKILL.md
moleapi-cli schema
moleapi-cli --database ./moleapi.db list requests --workspace Demo
```

The Markdown export is a standalone standard `SKILL.md`; the JSON bundle additionally contains optional Codex interface metadata at `agents/openai.yaml`. The repository's full source lives at [tools/skills/moleapi](../tools/skills/moleapi/SKILL.md). Export only to the caller's explicit destination; no global agent installation or service contact occurs automatically. Request metadata exposes identifiers/methods/collections without exporting request URL/body/auth values. Clap now rejects runs without either input/workspace and snippet calls with only one of workspace/request before any storage initialization. Schema includes actual option conflicts and value cardinality from Clap.

Generate a Git-triggered test configuration for GitHub Actions, GitLab CI or Jenkins using shared presets:

```json
{
  "provider": "github",
  "source": {"kind": "remote", "server": "https://api.example.com", "workspace": "WORKSPACE_ID"},
  "collection": "COLLECTION_ID",
  "environment": "ENVIRONMENT_ID",
  "dataset": "DATASET_ID",
  "notifications": {"mode": "silent"},
  "reporter": "junit",
  "language": "en",
  "secret_name": "MOLEAPI_TOKEN",
  "branches": ["main"]
}
```

```sh
moleapi-cli ci --config ci.json
moleapi-cli ci --config ci.json --output moleapi-tests.yml
```

Set `provider` to `github`, `gitlab` or `jenkins`. For a repository collection, use `source: {kind: "file", path: "collection.json", format: "postman"}` and choose its collection/scenario ID or unique name. Native workspace exports preserve IDs; other converters may require selecting imported names. File runs are silent. Choose a saved dataset or `data_file` plus `data_format` (`csv`/`json`), optional1–100 `iterations`, collection-only `requests`, report `junit`/`json`/`html`/`csv` and exact branch names. Remote notifications can be `silent`, `defaults` or `{mode: "selected", ids: [...]}`. Credentials are configured in the CI platform under the reference name and are not written into the configuration.

Add `"variables_secret": "API_RUN_VARIABLES_JSON"` to the preset JSON to bind private request-variable JSON. `secret_name` authenticates the CLI to MoleAPI; `variables_secret` supplies request values and must use a separate secret reference. File-mode presets can also use this option.

在配置 JSON 中添加 `variables_secret` 可引用请求变量密钥；`secret_name` 用于登录 MoleAPI 服务端，两者不能共用引用。仓库文件模式同样支持运行时变量注入。

Place the GitHub file under `.github/workflows/`; the default job uses a self-hosted Linux runner with `moleapi-cli` already installed. Use the GitLab file as `.gitlab-ci.yml` or include it and retain a `test` stage. Set Jenkins's Pipeline from SCM script path to `Jenkinsfile.moleapi`; exact branch selection requires a multibranch job, and credential/artifact/JUnit plugins must be present. Reports are archived after failures, with native failure status retained. Configuration generation contacts no services and executes no requests. Source/native report privacy and external CI integration limits are recorded in [CI-PRESET-COVERAGE.md](CI-PRESET-COVERAGE.md).

设置界面也可以选择集合或场景、环境、数据集、接口筛选、通知、报告和触发分支，预览并下载三种平台的配置。先保存工作区；本地文件模式可以另行导出隐藏私密值的原生集合快照。生成不会执行请求、注册远程任务或安装执行器。CI 执行器需先安装 CLI，敏感凭据通过平台密钥配置。
