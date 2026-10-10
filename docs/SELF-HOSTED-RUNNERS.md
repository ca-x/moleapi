# Self-hosted runners / 自托管执行器

A registered native CLI agent pulls owned jobs from a hosted MoleAPI service, executes them through an isolated local Rust router and returns native redacted reports. HTTP calls originate from the agent's machine, so it can reach authorized APIs on its network. The agent makes outbound service requests; the service does not need an inbound connection to the runner. Multiple registered runner IDs provide separate execution slots; one ID holds at most one live lease.

原生 CLI Agent 从托管服务领取所属账户的任务，在本机隔离的 Rust 执行环境中运行，再回传脱敏报告。接口请求从 Agent 所在机器发出，因此可以访问该机器可达的内网接口。Agent 主动连接托管端，不需要托管端反向访问 Agent。每个注册 ID 同时持有一个任务租约，注册多个执行器可提供多个执行槽位。

## Register and queue / 注册与排队

Use a login session or owned personal API token through the existing credential file/environment options. Registered agents are trusted with selected workspace request credentials; use a dedicated account when isolating runner access. Workspace-scoped runner credentials and team roles remain outstanding.

通过现有文件或系统环境变量选项传入登录凭据或个人 API 令牌。执行器会收到所选工作区的请求凭据，因此应部署在可信机器上；需要账户隔离时使用独立账户。目前继承账户的资源权限，工作区级 Runner 凭据和团队角色仍待补齐。

```sh
moleapi-cli --server https://api.example.com --token-env MOLEAPI_TOKEN runner register --name "LAN worker"
moleapi-cli --server https://api.example.com --token-env MOLEAPI_TOKEN runner list
moleapi-cli --server https://api.example.com --token-env MOLEAPI_TOKEN list workspaces
moleapi-cli --server https://api.example.com --token-env MOLEAPI_TOKEN list collections --workspace WORKSPACE_ID
```

Create `task.json` with actual runner/resource IDs and the saved workspace revision:

```json
{
  "runner_id": "REGISTERED_RUNNER_ID",
  "expected_revision": 1,
  "selection": {
    "collection_id": "COLLECTION_ID",
    "environment_id": "DEVELOPMENT_ENVIRONMENT_ID",
    "iterations": 1,
    "notification_ids": []
  },
  "max_attempts": 1
}
```

Selection also accepts `scenario_id`, `dataset_id` and `request_ids`. Use a scenario or collection request filters separately. A saved dataset must include source data; iteration bounds and subtree filters are validated before queueing. Missing/changed saved revisions are rejected. Notification IDs refer to configured owned targets and are sent through the hosted report notification path; the offline agent does not run stored schedule or notification loops.

选择配置还支持场景、已保存数据集和接口 ID 筛选；场景与接口筛选不能同时使用。数据集需要包含实际源数据。服务端在排队前校验迭代次数、接口所属集合子树和工作区修订。通知 ID 对应所属账户已经配置的目标，由托管端报告模块处理；离线 Agent 不启动已保存的定时任务或通知投递循环。

```sh
moleapi-cli --server https://api.example.com --token-env MOLEAPI_TOKEN runner queue --workspace WORKSPACE_ID --config task.json
moleapi-cli --server https://api.example.com --token-env MOLEAPI_TOKEN runner tasks --workspace WORKSPACE_ID
moleapi-cli --server https://api.example.com --token-env MOLEAPI_TOKEN runner agent --id REGISTERED_RUNNER_ID
```

`--once` handles one task or exits0 if the queue is empty. Continuous agents poll every2seconds, heartbeat every20seconds and renew a90second lease. Control requests time out after10seconds. Use `--variables-file` or `--variables-env` to provide existing private temporary/project/collection/environment overrides; the JSON envelope is documented in [CLI.md](CLI.md). These values do not change canonical hosted workspace variables. Each task uses a private temporary SQLite database removed when execution finishes.

`--once` 处理一个任务，无任务时返回0；常驻模式每2秒领取任务、每20秒发送心跳，租约有效期90秒，控制请求超时10秒。私密变量可通过现有 `--variables-file` 或 `--variables-env` 注入，格式见命令行文档；它们不写回托管工作区。每次执行使用独立的临时 SQLite 数据库，执行后清理。

## Cancellation, recovery and reports / 取消、恢复和报告

```sh
moleapi-cli --server https://api.example.com --token-env MOLEAPI_TOKEN runner cancel --workspace WORKSPACE_ID --id TASK_ID
moleapi-cli --server https://api.example.com --token-env MOLEAPI_TOKEN runner set --id REGISTERED_RUNNER_ID --name "LAN worker" --enabled false --revision RUNNER_REVISION
moleapi-cli --server https://api.example.com --token-env MOLEAPI_TOKEN report --workspace WORKSPACE_ID --id REPORT_ID --format junit --output report.xml
```

Ctrl-C cancels current local execution, submits cancellation when the service is available, then exits130. Lost/denied heartbeat also cancels local execution; server-side cancellation or revocation is observed at the next heartbeat. A cancelled/disabled/revoked task cannot extend or complete its live lease. Explicit token revoke/logout invalidates queued owner jobs; subsequent jobs use the new credential revision. Future agent calls still require a valid credential.

按 Ctrl-C 会取消本地运行，服务端可用时回传取消结果，并以130退出。续租失败同样停止本地任务；服务端取消或撤销通常在下一次心跳时被 Agent 发现。禁用、取消或撤销后不能续租或提交活动任务。令牌撤销和退出登录使此前排队的账户任务失效，新任务使用新的凭据修订；Agent 的后续调用仍需要有效凭据。

The default `max_attempts: 1` does not automatically replay a lost task. Explicit values2–3 allow another claim after lease expiry; requests can run more than once, including requests with side effects. Expired attempts are reconciled when the agent claims work. Retry completion acknowledgements are idempotent and retain one report/notification transaction. If acknowledgement still fails, the CLI reports it and exits2; inspect task metadata before creating a replacement task.

默认只尝试一次，不自动重放失联任务；显式配置2–3次允许租约过期后重新领取，接口可能被重复调用，也包括有副作用的接口。失效任务在执行器再次领取时处理。重复回传采用幂等提交，不会生成重复报告；持续无法确认提交时 CLI 会提示并返回2，应先查看任务状态再决定是否重建任务。

A task marked `completed` means its result was accepted; inspect report assertions to determine test success. `--once` exits1 for failed/empty/stopped executions,0 for passing nonempty runs. Execution errors without a report use task status `failed`. Task metadata omits snapshots, private values and lease tokens. Reports use the existing source/variable redaction and JSON/CSV/JUnit/bilingual HTML exporters; worker-local history IDs are discarded. Response-detail history upload remains outstanding. Each account allows100 runners, each workspace100 pending tasks and the latest100 terminal metadata records.

`completed` 表示已接收执行结果，断言是否通过请查看报告。单次模式对失败、空运行或中止返回1，对非空且通过的运行返回0；无法产生报告的执行错误记为 `failed`。任务列表不含快照、私密值和租约令牌。报告沿用原有脱敏和 JSON/CSV/JUnit/中英文 HTML 导出，丢弃本地历史 ID；响应详情历史回传仍待实现。每账户最多100个执行器，每工作区最多100个待执行任务并保留最新100份终态任务记录。

## Current boundary / 当前边界

API/CLI task execution is implemented. Scheduled-run targeting, bilingual desktop/web runner management, labels/group scheduling, workspace-scoped credentials, service installation, history-response uploads, packaging and PostgreSQL/MySQL/platform deployment verification remain active requirements. Binary and Docker releases will use GitHub Actions after functional completion; no local Docker build was used. This module is not evidence of complete395-capability parity or a published release.

已实现 API/CLI 任务执行。定时任务选择 Runner、中英文桌面和网页管理、标签分组调度、工作区级凭据、服务安装、响应历史回传及数据库和平台验证仍在后续范围内。功能完成后使用 GitHub Actions 发布二进制和 Docker，本地不构建 Docker；当前不代表完整功能矩阵或新版已发布。
