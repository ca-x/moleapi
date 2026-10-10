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

GitHub Actions can invoke an installed CLI without Docker:

```yaml
- name: Run API smoke tests
  env:
    MOLEAPI_TOKEN: ${{ secrets.MOLEAPI_TOKEN }}
  run: >-
    moleapi-cli --server https://api.example.com --token-env MOLEAPI_TOKEN
    run --workspace Demo --collection Smoke --ci --no-notifications
    --report report.xml --reporter junit
- uses: actions/upload-artifact@v4
  if: always()
  with:
    name: api-report
    path: report.xml
```

GitLab CI can run the same command and attach `report.xml` under `artifacts:reports:junit`. Jenkins can run it in a shell stage and publish `junit 'report.xml'` in an always/post step. Supply credentials through the CI secret store. CLI installation/packaging is deferred until functionality is complete. These commands do not claim complete Newman compatibility, external custom reporters or full matrix completion.
