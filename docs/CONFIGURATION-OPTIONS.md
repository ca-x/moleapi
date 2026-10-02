# 具体 UI 与配置选项

这些记录把已核实的菜单/字段/枚举和已知套餐、平台条件单独保存；它们是完整公开目录的重点提取，不宣称穷尽登录后所有 UI 控件。`source_anchor=null` 表示没有实测远端锚点，原文行号和章节可定位。更多配置项可按 feature-options.json 的公开文档层级继续定位。

## CTRL-001 · apifox · 请求 Body 格式

**选项：** form-data, x-www-form-urlencoded, JSON, XML, raw, binary, GraphQL, Msgpack

**入口：** 未现场观察；见官方说明

**来源：** https://docs.apifox.com/request-params-and-body.md

## CTRL-002 · apifox · Auth 类型

**选项：** 从父级继承, No Auth, API Key, Bearer Token, JWT, Basic Auth, Digest Auth, OAuth 1.0, OAuth 2.0, Hawk Authentication, Kerberos, NTLM, Akamai EdgeGrid

**入口：** 接口 → Auth → 类型

**来源：** https://docs.apifox.com/authorization-types.md

## CTRL-003 · postman · 基础 Authorization 类型

**选项：** No Auth, API Key, Bearer Token, JWT Bearer, Basic Auth

**入口：** Request → Authorization → Auth Type

**来源：** https://learning.postman.com/docs/use/send-requests/authorization/authorization-types

**条件：** 更多授权类型见各自官方页面，不以此基本类型列表表示完整下拉菜单。

## CTRL-004 · apifox · 变量作用域（高到低）

**选项：** 临时, 测试数据, 环境, 模块, 项目全局, 团队全局

**入口：** 环境管理

**来源：** https://docs.apifox.com/global-environment-session-variables.md

**条件：** 临时 > 测试数据 > 环境 > 模块 > 项目全局 > 团队全局；远程值同步；本地值不共享，设置后覆盖远程值。

## CTRL-005 · apifox · 外部 Vault provider

**选项：** HashiCorp Vault, Azure Key Vault, AWS Secrets Manager

**入口：** 团队资源/项目 → 密钥库提供商

**来源：** https://docs.apifox.com/vault-secrets.md

**条件：** 商业旗舰版；值加密保存本地，不共享；名称和元数据可共享。

## CTRL-006 · apifox · 场景控制节点

**选项：** 分组, ForEach 循环, For 循环, 条件分支, 等待时间

**入口：** 自动化测试 → 场景用例 → 添加步骤

**来源：** https://docs.apifox.com/flow-control-conditions.md

## CTRL-007 · apifox · 条件判断运算

**选项：** 等于, 不等于, 存在, 不存在, 小于, 小于或等于, 大于, 大于或等于, 正则匹配, 包含, 不包含, 为空, 不为空, 属于集合, 不属于集合

**入口：** 未现场观察；见官方说明

**来源：** https://docs.apifox.com/flow-control-conditions.md

## CTRL-008 · apifox · 工作台模块

**选项：** 接口管理, 自动化测试, 分享文档, 请求历史, 项目设置, 邀请成员

**入口：** 未现场观察；见官方说明

**来源：** https://docs.apifox.com/page-layout.md

## CTRL-009 · postman · 侧栏视图

**选项：** Items, Services, History, Local Files

**入口：** 未现场观察；见官方说明

**来源：** https://learning.postman.com/docs/getting-started/basics/navigating-postman

## CTRL-010 · postman · 默认资源分组

**选项：** Collections, Environments, Documents, Specs, SDKs, Datasets, Flows

**入口：** Sidebar → Customize sidebar

**来源：** https://learning.postman.com/docs/getting-started/basics/navigating-postman

## CTRL-011 · postman · 协议/客户端类型

**选项：** HTTP/REST, AI Requests, Data, GraphQL, gRPC, MCP, MQTT, SOAP, UDS/Named Pipes, Webhook, WebSocket/Socket.IO

**入口：** 未现场观察；见官方说明

**来源：** https://learning.postman.com/docs/use/send-requests/protocols/protocols

**条件：** 不同协议提供独立客户端，不能用统一 HTTP response 字段代替。

## CTRL-012 · postman · Spec Hub 规范格式

**选项：** OpenAPI, AsyncAPI, protobuf 2/3, GraphQL, Smithy 2.0

**入口：** 未现场观察；见官方说明

**来源：** https://learning.postman.com/docs/design-apis/specifications/overview

## CTRL-013 · postman · Git/云工作方式

**选项：** Local View, Cloud View

**入口：** 未现场观察；见官方说明

**来源：** https://learning.postman.com/docs/use/native-git/overview

**条件：** Native Git 仅桌面端；本地 Git 文件与显式 CI/云发布边界需独立处理。

## CTRL-014 · postman · 集合格式

**选项：** 3.0.0 多文件 YAML, 2.1.0 JSON 兼容导出

**入口：** 未现场观察；见官方说明

**来源：** https://learning.postman.com/docs/use/use-collections/collections-schemas

**条件：** Newman 不能直接执行 3.0，执行 2.1 导出；3.0 使用 Postman CLI。

## CTRL-015 · postman · Dataset 数据源

**选项：** Postman Cloud Store, Local File, MySQL, Postgres, SQL Server, JDBC Source, Add with AI

**入口：** Sidebar → + → Dataset

**来源：** https://learning.postman.com/docs/tests-and-scripts/datasets/create-datasets

**条件：** Dataset: Solo/Team/Enterprise；Live database: Team/Enterprise；Custom JDBC: Enterprise；Desktop 全部；Web 仅 Cloud Store/MySQL/Postgres。

## CTRL-016 · postman · 数据文件类型

**选项：** CSV, JSON, .xlsx, .xls, .ods

**入口：** 未现场观察；见官方说明

**来源：** https://learning.postman.com/docs/tests-and-scripts/datasets/create-datasets

**条件：** 多表格工作表分别成为数据源；Spreadsheet 不提供预览但可以添加。

## CTRL-017 · postman · JDBC URL patterns

**选项：** MySQL, Postgres, SQL Server, Oracle, Generic

**入口：** Dataset → JDBC Source → Driver → Connection URL

**来源：** https://learning.postman.com/docs/tests-and-scripts/datasets/create-datasets

**条件：** 需要 JDBC driver JAR 和可用 Java runtime；JDBC View 使用原数据库 SQL dialect，单数据源，不可跨来源 join。

## CTRL-018 · postman · 数据库/SSH 连接字段

**选项：** Host, Port, Database, Username, Password, Table, Schema, SSH host, SSH port, SSH username, SSH private key, SSH host key, Skip SSH host key verification

**入口：** Dataset → Data Source → Connection

**来源：** https://learning.postman.com/docs/tests-and-scripts/datasets/create-datasets

**条件：** Schema 字段适用 Postgres/SQL Server；SSH 的源必须能从 SSH server 网络访问。

## CTRL-019 · postman · 交互式 Data request source

**选项：** Local file, Remote file, MySQL, PostgreSQL

**入口：** Sidebar → + → Data → address bar

**来源：** https://learning.postman.com/docs/use/send-requests/protocols/data/create-data-request

**条件：** 桌面或启用 Desktop Agent 的 Web；Local file 仅桌面；Read-only 设置阻止写入/改 schema。

## CTRL-020 · postman · 账号 Region Preference

**选项：** Always Ask for Region Selection, Use EU Region by Default

**入口：** Desktop → Help → Region Preference for New Accounts

**来源：** https://learning.postman.com/docs/administration/enterprise/about-eu-data-residency

**条件：** Enterprise EU 计划；只改变新账号认证入口，不改变已存在账号；EU 文档列出集成/Partner Workspace/BYOK runs/Live Sessions/API Network/Fern 等能力例外。

## CTRL-021 · postman · Newman CLI 选项分组

**选项：** Basic options, Setup options, Request options, SSL options, Output options, More configuration options, Exit status, Data file example

**入口：** 未现场观察；见官方说明

**来源：** https://learning.postman.com/docs/reference/newman-cli/newman-options

**条件：** 这里列官方选项章节；具体 flags/默认值查对应正文，不把章节名当成参数值。

## CTRL-022 · postman · Newman 内置 Reporter

**选项：** Use built-in reporters, About built-in reporters, Configure built-in reporters, CLI reporter, JSON reporter, JUnit reporter

**入口：** 未现场观察；见官方说明

**来源：** https://learning.postman.com/docs/reference/newman-cli/newman-built-in-reporters

**条件：** 这里列官方选项章节；具体 flags/默认值查对应正文，不把章节名当成参数值。

## CTRL-023 · postman · Postman CLI 选项分组

**选项：** Command categories, Basic CLI commands, Authentication commands, Workspace commands, Team commands, Collection commands, Dependency commands, Environment commands, Globals commands, Request commands, Monitoring and performance commands, Application commands, Flows commands, API governance commands, Mock server commands, Dataset commands, Search commands, Simulator commands, Publish API versions commands, SDK commands, Coding agent commands, Context Graph commands, Skills commands, Webhooks commands, Usage analytics, Get started

**入口：** 未现场观察；见官方说明

**来源：** https://learning.postman.com/docs/postman-cli/postman-cli-options

**条件：** 这里列官方选项章节；具体 flags/默认值查对应正文，不把章节名当成参数值。
