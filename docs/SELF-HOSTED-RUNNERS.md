# Self-hosted runners / 自托管执行器

A registered native CLI agent pulls owned jobs from a hosted MoleAPI service, executes them through an isolated local Rust router and returns native redacted reports. HTTP calls originate from the agent's machine, so it can reach authorized APIs on its network. The agent makes outbound service requests; the service does not need an inbound connection to the runner. Multiple registered runner IDs provide separate execution slots; one ID holds at most one live lease.

原生 CLI Agent 从托管服务领取所属账户的任务，在本机隔离的 Rust 执行环境中运行，再回传脱敏报告。接口请求从 Agent 所在机器发出，因此可以访问该机器可达的内网接口。Agent 主动连接托管端，不需要托管端反向访问 Agent。每个注册 ID 同时持有一个任务租约，注册多个执行器可提供多个执行槽位。

## Register and queue / 注册与排队

Use an owner login session to create a runner credential for exactly one runner and workspace. Queueing and runner management use the owner session or PAT; `runner agent` requires the scoped credential through its own private `--token-file` or `--token-env`. Registered agents receive the selected workspace request credentials when they claim a task, so deploy them only on trusted machines. Team roles remain outstanding.

先用账户登录会话为指定执行器和工作区创建专用凭据。排队及执行器管理可使用账户会话或 PAT；`runner agent` 必须通过独立的私有 `--token-file` 或 `--token-env` 使用该专用凭据。执行器领取任务时仍会收到所选工作区的请求凭据，应只部署在可信机器上。团队角色仍待补齐。

Hosted runner management accepts optional `labels` on `POST /api/runners` and `PATCH /api/runners/{id}`. Labels are unique lowercase letters/digits/hyphens, 1–32 characters each, at most 10 per runner. Omitting labels on an update preserves existing labels; `[]` clears them. Existing runner records without labels read as an empty list. In the hosted UI, enter comma-separated labels when registering or editing a runner. The dispatch selector accepts a direct runner ID or a label group. A label group binds each new task to the enabled member with the fewest queued or leased tasks; runner ID breaks ties. The bound task never moves to another runner or to the hosted server if that machine goes away. In the CLI, repeat `--label` on `runner register` or `runner set`; omit it on `set` to preserve labels, or use `--clear-labels` to remove all labels. The server rejects invalid and duplicate labels.

托管执行器可通过注册/修改 API 的 `labels` 字段添加标签，也可在网页端用逗号分隔输入和筛选。CLI 注册和修改命令可重复使用 `--label`；修改时省略表示保留现有标签，`--clear-labels` 表示清空。手动任务和计划任务可选具体执行器，或选一个标签组。入队时服务端从同标签的已启用执行器中选择待处理任务最少者，相同数量按 ID 排序；任务绑定后不会静默转移到其他执行器或服务端。

For CLI group dispatch, replace `runner_id` in `task.json` with `"runner_label":"lan"`; specify exactly one selector. Schedules use nullable `runner_id` or `runner_label`, with both null meaning local service execution. If a label has no enabled member when a scheduled occurrence starts, that occurrence fails explicitly. Disabling a runner cancels its queued and active tasks.

```sh
moleapi-cli --server https://api.example.com --token-env MOLEAPI_TOKEN runner register --name "LAN worker" --label lan --label linux
moleapi-cli --server https://api.example.com --token-env MOLEAPI_TOKEN runner list
moleapi-cli --server https://api.example.com --token-env MOLEAPI_TOKEN list workspaces
moleapi-cli --server https://api.example.com --token-env MOLEAPI_TOKEN list collections --workspace WORKSPACE_ID
```

Issue the agent credential with a login session. The CLI writes its secret to a private file and prints metadata only. `list` never reveals the secret; rotate by creating a replacement and revoking the old credential. The hosted Collection tests runner panel offers the same one-time issue and confirmed revoke flow for the current workspace. A PAT cannot manage runner credentials. Do not pass bearer values as command arguments.

使用登录会话签发 Agent 凭据。CLI 将原文写入私有文件，标准输出只打印元数据；列表不会再次显示原文。轮换时先创建新凭据，再撤销旧凭据。托管网页的集合测试执行器面板也支持对当前工作区一次性签发和确认撤销。PAT 不能管理执行器凭据。不要把令牌原文放入命令参数。

```sh
moleapi-cli --server https://api.example.com --token-env MOLEAPI_TOKEN runner credentials create --runner REGISTERED_RUNNER_ID --workspace WORKSPACE_ID --name "LAN workspace" --output agent-token.txt
moleapi-cli --server https://api.example.com --token-env MOLEAPI_TOKEN runner credentials list --runner REGISTERED_RUNNER_ID
moleapi-cli --server https://api.example.com --token-env MOLEAPI_TOKEN runner credentials revoke --runner REGISTERED_RUNNER_ID --id CREDENTIAL_ID
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
moleapi-cli --server https://api.example.com --token-file agent-token.txt runner agent --id REGISTERED_RUNNER_ID
```

`--once` handles one task or exits0 if the queue is empty. Continuous agents poll every2seconds, heartbeat every20seconds and renew a90second lease. Control requests time out after10seconds. Use `--variables-file` or `--variables-env` to provide existing private temporary/project/collection/environment overrides; the JSON envelope is documented in [CLI.md](CLI.md). These values do not change canonical hosted workspace variables. Each task uses a private temporary SQLite database removed when execution finishes.

`--once` 处理一个任务，无任务时返回0；常驻模式每2秒领取任务、每20秒发送心跳，租约有效期90秒，控制请求超时10秒。私密变量可通过现有 `--variables-file` 或 `--variables-env` 注入，格式见命令行文档；它们不写回托管工作区。每次执行使用独立的临时 SQLite 数据库，执行后清理。

## Supervised Linux user service / Linux 用户级服务

After saving the scoped agent credential in a regular `0600` file, preview the user unit, then install it. An optional `--variables-file` must also deny group and other access. The service restarts on failure, uses the existing outbound agent loop and has no listening port. Installation writes only file paths to the unit, never the bearer contents. `install --replace` updates a MoleAPI-managed unit and restarts it; modified or foreign units are refused. Service commands do not contact the hosted API.

将执行器专用凭据保存为普通 `0600` 文件后，先预览 user unit，再安装。可选的 `--variables-file` 也必须拒绝组和其他用户访问。服务异常退出会重新启动，仅运行现有主动连接的 Agent，不监听端口。unit 中只有文件路径，没有凭据原文。`install --replace` 仅更新本工具管理的 unit 并重启；被修改或非本工具的 unit 会被拒绝。服务命令本身不访问托管 API。

```sh
moleapi-cli --server https://api.example.com --token-file agent-token.txt runner service print --id REGISTERED_RUNNER_ID
moleapi-cli --server https://api.example.com --token-file agent-token.txt runner service install --id REGISTERED_RUNNER_ID
moleapi-cli runner service status --id REGISTERED_RUNNER_ID
moleapi-cli runner service uninstall --id REGISTERED_RUNNER_ID
```

These commands target Linux systemd user services. macOS launchd and Windows supervision remain separate platform work; this implementation and its test code have not yet been run through the final verification gate.

## Cancellation, recovery and reports / 取消、恢复和报告

```sh
moleapi-cli --server https://api.example.com --token-env MOLEAPI_TOKEN runner cancel --workspace WORKSPACE_ID --id TASK_ID
moleapi-cli --server https://api.example.com --token-env MOLEAPI_TOKEN runner set --id REGISTERED_RUNNER_ID --name "LAN worker" --enabled false --revision RUNNER_REVISION
moleapi-cli --server https://api.example.com --token-env MOLEAPI_TOKEN runner set --id REGISTERED_RUNNER_ID --name "LAN worker" --enabled true --revision RUNNER_REVISION --label lan --label ci
moleapi-cli --server https://api.example.com --token-env MOLEAPI_TOKEN runner set --id REGISTERED_RUNNER_ID --name "LAN worker" --enabled true --revision RUNNER_REVISION --clear-labels
moleapi-cli --server https://api.example.com --token-env MOLEAPI_TOKEN report --workspace WORKSPACE_ID --id REPORT_ID --format junit --output report.xml
```

Ctrl-C cancels current local execution, submits cancellation when the service is available, then exits130. Lost/denied heartbeat also cancels local execution; server-side cancellation or revocation is observed at the next heartbeat. Revoking a runner credential immediately cancels its active lease across hosted instances without cancelling unrelated credentials or queued tasks. A cancelled/disabled/revoked task cannot extend or complete its live lease. Explicit owner token revoke/logout still invalidates queued owner jobs through the account credential revision.

按 Ctrl-C 会取消本地运行，服务端可用时回传取消结果，并以130退出。续租失败同样停止本地任务；服务端取消或撤销通常在下一次心跳时被 Agent 发现。撤销执行器凭据会跨托管实例立即取消其活动租约，不影响其他凭据或排队任务。禁用、取消或撤销后不能续租或提交活动任务。撤销账户令牌及退出登录仍会通过账户凭据修订使此前排队的账户任务失效。

The default `max_attempts: 1` does not automatically replay a lost task. Explicit values2–3 allow another claim after lease expiry; requests can run more than once, including requests with side effects. Expired attempts are reconciled when the agent claims work. Retry completion acknowledgements are idempotent and retain one report/notification transaction. If acknowledgement still fails, the CLI reports it and exits2; inspect task metadata before creating a replacement task.

默认只尝试一次，不自动重放失联任务；显式配置2–3次允许租约过期后重新领取，接口可能被重复调用，也包括有副作用的接口。失效任务在执行器再次领取时处理。重复回传采用幂等提交，不会生成重复报告；持续无法确认提交时 CLI 会提示并返回2，应先查看任务状态再决定是否重建任务。

A task marked `completed` means its result was accepted; inspect report assertions to determine test success. `--once` exits1 for failed/empty/stopped executions,0 for passing nonempty runs. Execution errors without a report use task status `failed`. Task metadata omits snapshots, private values and lease tokens. Reports use the existing source/variable redaction and JSON/CSV/JUnit/bilingual HTML exporters; worker-local history IDs are discarded. Response-detail history upload remains outstanding. Each account allows100 runners, each workspace100 pending tasks and the latest100 terminal metadata records.

`completed` 表示已接收执行结果，断言是否通过请查看报告。单次模式对失败、空运行或中止返回1，对非空且通过的运行返回0；无法产生报告的执行错误记为 `failed`。任务列表不含快照、私密值和租约令牌。报告沿用原有脱敏和 JSON/CSV/JUnit/中英文 HTML 导出，丢弃本地历史 ID；响应详情历史回传仍待实现。每账户最多100个执行器，每工作区最多100个待执行任务并保留最新100份终态任务记录。

## Current boundary / 当前边界

API/CLI execution and bilingual hosted web management are implemented. In Collection tests, open Self-hosted runners to register/rename/enable/disable runners, queue a saved collection or scenario, cancel tasks and open reports. Dispatch saves dirty workspace changes first and uses that returned revision; browser local variable overrides are excluded. Offline desktop management directs users to the hosted website while retaining direct local execution.

Schedules can select Current server or an owned enabled runner. `runner_id` is optional in schedule definitions; absent/null preserves existing local/server execution. Each due occurrence atomically queues one single-attempt task at the current saved workspace revision and persists `running.runner_task_id`. Offline runners leave tasks queued without falling back to the server. Server restart preserves dispatches. Schedule cancellation, edits and deletion cancel the associated task; lease expiry fails without replay. Completed tasks populate schedule history and report links through the existing notification path. Cancellation still takes effect on an agent at its next heartbeat.

Label-group dispatch and response-history upload implementations and tests are pending final execution verification. The native agent fetches its locally saved, privacy-screened response entries and uploads up to 8 MiB with the completion report. The hosted service screens them again, creates new owned history links atomically with the report, and drops links for entries not uploaded. CLI completion metadata includes `response_history_omitted`; it never prints response bodies. In the Tauri app, connect to the hosted account in Sync settings, sync the selected workspace, then open Settings > Hosted runner management. That panel uses the remote workspace revision and source, with runner/credential/task/schedule actions and report response preview through an IPC-only allowlisted gateway. It does not replace local collection execution or expose the stored login token to JavaScript. Native management and Linux service installation are coded and await runtime tests. macOS/Windows supervision, packaging and PostgreSQL/MySQL/platform deployment verification remain active requirements. Binary and Docker releases will use GitHub Actions after functional completion; no local Docker build was used. This module is not evidence of complete395-capability parity or a published release.

已实现 API/CLI 执行、中英文托管网页管理和定时任务选择 Runner。集合测试页可注册、重命名、启停执行器，派发保存的集合或场景、取消任务并查看报告。派发前保存工作区，使用返回的修订，不附加浏览器本地覆盖。离线桌面继续直接执行集合，Runner 管理通过托管网页进行。

每次计划以当前保存修订原子生成一个单次任务并持久记录任务 ID，等待离线机器连接，不退回服务端执行。重启后继续协调，取消、修改和删除计划会取消关联任务；租约到期记为失败，不重放请求。完成结果写回计划记录和报告入口，通知沿用现有模块。标签组派发、响应历史回传、Tauri 原生托管管理及 Linux 用户级服务安装均已写入代码、仍待最后执行测试。桌面端先通过同步设置连接账户并同步工作区，再在设置中打开“托管执行器管理”；此区域使用远端修订，不改动离线执行。macOS/Windows 服务、数据库和平台验证仍在范围内。功能完成后使用 GitHub Actions 发布，本地不构建 Docker；当前不代表完整功能矩阵或新版已发布。
