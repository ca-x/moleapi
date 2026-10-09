<p align="center"><img src="assets/brand/logo.png" width="128" alt="MoleAPI Logo"></p>
<h1 align="center">MoleAPI</h1>
<p align="center">可自托管的 API 工作台与独立离线桌面客户端。</p>
<p align="center"><a href="README.md">English</a> · 简体中文</p>

MoleAPI 使用 Rust/Axum/SeaORM、React/Radix UI/CodeMirror 和 Tauri。默认 SQLite，也支持 PostgreSQL 与 MySQL；服务端嵌入前端、字体和编辑器资源，运行时无需 Node.js。桌面端通过进程内 IPC 使用本地 SQLite，可独立运行并按需连接自托管服务器同步。

**项目仍在开发，尚未完整覆盖 Apifox／Postman。** [实现与验证状态](docs/IMPLEMENTATION-STATUS.md)、[协议覆盖](docs/PROTOCOL-COVERAGE.md)和[代码生成覆盖](docs/SNIPPET-COVERAGE.md)记录真实证据与缺口。已有专用 HTTP、SSE、WebSocket、GraphQL、gRPC、Socket.IO、MQTT、SOAP、MCP、A2A、TCP/TLS、Data SQL 专用客户端和 Webhook 接收器，以及工作区、集合、环境／作用域变量、脚本、历史、断言、基础 Mock 和集合运行器。多语言请求示例与完整 SDK／服务端生成是不同能力，后者仍在推进。

集合测试支持临时或保存的 CSV／JSON 数据集、带类型的迭代数据、多轮运行和停止操作。可复用数据集随工作区保存、版本管理和同步，默认备份隐藏私密源。参见[运行器能力](docs/DATA-RUNNER-COVERAGE.md)及[可复用数据集覆盖](docs/SAVED-DATASETS-COVERAGE.md)。

集合脚本可通过 pm.execution 控制跳转、循环、结束本轮或跳过请求。参见[流程行为与限制](docs/SCRIPT-FLOW-COVERAGE.md)。

响应检查支持 Header／正则／JSONPath／XPath／JSON Schema 断言，复用成熟 Rust 库。参见[断言能力与限制](docs/ADVANCED-ASSERTION-COVERAGE.md)。

## 软件截图

截图来自实际运行的自托管界面，使用合成测试数据；与桌面端共用前端，截图不代表托盘或安装包已通过实机验证。

![英文 API 工作台](docs/images/workbench-en.png)

![中文协议工作台](docs/images/protocol-zh-CN.png)

![Data SQL 查询结果](docs/images/data-en.png)

![中文 SQL 编辑器与数据结构浏览器](docs/images/data-zh-CN.png)

![OAuth2 授权与私有令牌管理](docs/images/oauth2-zh-CN.png)

![按工作区和开发环境隔离的私有 Cookie 管理](docs/images/cookies-zh-CN.png)

![OAuth1 自动授权与私有令牌库](docs/images/oauth1-zh-CN.png)

![与独立 HTTP 服务互通的 NTLMv2 认证](docs/images/ntlm-zh-CN.png)

![通过独立签名验证的 EdgeGrid 二进制请求](docs/images/edgegrid-zh-CN.png)

![通过独立 JOSE 验证的 ASAP ES512 认证](docs/images/asap-zh-CN.png)

![真实 DNS 覆盖保留 Host 的请求网络设置](docs/images/network-zh-CN.png)

![OpenAPI 项目生成与 Rust SDK 文件预览](docs/images/codegen-project-zh-CN.png)

OpenAPI 项目生成使用原生 Progenitor Rust 客户端引擎，以及固定版本、嵌入二进制的 OpenAPI Generator 多语言客户端/服务端生成器。Java 目标需显式配置 Java17+ 可执行文件（`MOLEAPI_CODEGEN_JAVA`），原生 Rust 不需要 Java。保留原始定义，默认隐藏凭据；ZIP 清单记录文件及校验和。参见[实际目标验证与剩余范围](docs/PROJECT-GENERATION-COVERAGE.md)。

![目录与 ZIP 输入的生成项目再生成合并](docs/images/codegen-regeneration-zh-CN.png)

再生成可以读取旧生成 ZIP 或快照，以及当前编辑目录／ZIP；保留用户文件，比较删除项，选择冲突处理后下载新的合并 ZIP，不覆盖原文件。

![OpenAPI 目录／ZIP 源包与入口选择](docs/images/codegen-source-bundle-zh-CN.png)

单个 HTTP 接口可生成与 Apifox／Postman 请求代码目录对应的23类语言／工具、55种库适配器，支持预览、复制和源文件下载：C、C#、Clojure、Dart、F#、Go、HTTP、Java、JavaScript、Julia、Kotlin、Node.js、Objective-C、OCaml、PHP、PowerShell、Python、R、Ruby、Rust、Shell、Swift 和 Postman CLI。可分别选择 HttpClient／RestSharp、Fetch／Axios、Requests／HTTPX、OkHttp／Unirest 等库。参见[完整语言／库目录](docs/REQUEST-CODE-PARITY.md)和[正文类型、依赖与运行证据](docs/SNIPPET-COVERAGE.md)。原始文件正文仅显示支持文件字节读取的适配器。

数据模型生成覆盖 Apifox 的19类语言：18种语言复用嵌入式 Quicktype，MySQL／PostgreSQL 建表语句复用 OpenAPI Generator。可选全部或单个模型、调整序列化与代码风格，再预览、复制、下载或再生成。参见[模型运行证据与剩余限制](docs/MODEL-GENERATION-COVERAGE.md)。

![数据模型生成与 C# 预览](docs/images/codegen-models-zh-CN.png)

OpenAPI Generator 目标支持导入自定义 Mustache 模板及静态资源的目录、ZIP 或生成快照，可预览、编辑及配置接口／模型／文档／测试／额外文件输出，也可恢复上游模板；导出保留模板原文、二进制资源、映射和校验和。参见[模板功能证据与剩余限制](docs/TEMPLATE-GENERATION-COVERAGE.md)。

HTTP/SOAP、SSE/WebSocket/GraphQL、gRPC、TCP/TLS、MQTT、Socket.IO、A2A 和 MCP HTTP 请求支持显式 HTTP/SOCKS 代理、绕过规则、自定义 CA、PEM/加密 PFX 客户端证书、DNS 覆盖和 HTTP/2。网络凭据会从历史与默认导出中脱敏；精确保留源配置需要显式包含敏感信息。参见[网络支持与当前组合限制](docs/specs/request-network.md)。

## 语言

软件提供简体中文和英文界面，在登录页或工作台设置中切换并记忆选择。请求名称、变量、规范原文、脚本、实际服务响应和生成代码属于用户数据，保持原文。原生托盘菜单跟随界面语言；系统原生对话框按操作系统能力显示。新增语言使用嵌入式国际化目录。

## 构建与运行服务端

需要 Rust1.94+、Node22+；运行已经构建的服务端只需二进制文件。

```bash
npm --prefix web ci
npm --prefix web run build
cargo build --locked --release -p moleapi-server
./target/release/moleapi-server --bind 127.0.0.1:8787
```

打开 http://127.0.0.1:8787，用首次启动日志中的初始化令牌创建账户。默认仅首次管理员注册；额外注册使用 --allow-registration。局域网接口调试可显式开启 --allow-private-network。

SQLite 默认文件为 ./data/moleapi.db。其他数据库通过 --database-url 或 MOLEAPI_DATABASE_URL 配置：

```bash
MOLEAPI_DATABASE_URL='postgresql://user:password@localhost:5432/moleapi' ./target/release/moleapi-server
MOLEAPI_DATABASE_URL='mysql://user:password@localhost:3306/moleapi' ./target/release/moleapi-server
```

[数据库规格](docs/specs/database.md)说明配置与备份。切换数据库 URL 不会自动迁移数据。

## 桌面端与 Docker

```bash
npm --prefix web run desktop:dev
npm --prefix web run desktop:build
docker compose pull
docker compose up --no-build -d
```

桌面构建需要 Tauri 系统开发依赖。Windows 配置 NSIS/MSI，macOS 配置 DMG，Linux 配置 deb/rpm/AppImage。原生托盘提供显示、隐藏、退出和窗口唤回；真实支持程度以当前平台验证记录为准。签名与公证需要密钥，不能将未签名产物称为已签名版本。

Docker 镜像只通过 GitHub Actions 的原生 amd64/arm64 runner 构建、验证与发布，本地拉取运行。开发镜像 ghcr.io/ca-x/moleapi:feat-application 可能落后于最新源代码，实际版本和平台以 Registry／Actions 产物为准，可用 MOLEAPI_IMAGE 选择发布版本。数据使用 /data 卷，也支持外部 PostgreSQL／MySQL。

## 验证与自动构建

```bash
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
npm --prefix web test
npm --prefix web run typecheck
```

GitHub Actions 检查源代码与真实数据库用例，构建多平台服务端、桌面安装包和 Docker。最新平台失败／未验证项目仍在状态文档中列明；某次绿色检查不等于全量功能或所有安装包已完成。

## 功能依据

- [完整功能矩阵](docs/FEATURE-MATRIX.md)、[能力模块](docs/CAPABILITY-MAP.md)、[协议与 Rust 库](docs/PROTOCOL-COVERAGE.md)
- [Apifox 公开目录](docs/APIFOX-CATALOG.md)、[Postman 公开目录](docs/POSTMAN-CATALOG.md)
- [配置选项](docs/CONFIGURATION-OPTIONS.md)、[UI 调研](docs/UI-RESEARCH.md)、[调研记录](docs/RESEARCH-NOTES.md)

透明鼹鼠 Logo 由项目发起者提供。竞品名称及原始资料归各自权利人所有，仅作为功能调研来源；软件不使用竞品品牌资产。

响应提取规则支持 JSON Pointer、JSONPath、XPath、正则、响应头和完整正文，写入当前选定开发环境、集合、项目或临时变量。规则在后置脚本前执行，后续集合请求可读取结果；共享工作区不会自动被覆盖。参见[响应提取范围与限制](docs/RESPONSE-EXTRACTION-COVERAGE.md)。

可保存的测试场景引用原接口，支持自定义步骤顺序、分组、启用状态和重复请求。在测试页创建、编辑、排序并保存，使用当前环境及数据集／迭代设置运行。参见[场景能力与剩余范围](docs/SCENARIO-COVERAGE.md)。

场景步骤支持 JavaScript 布尔条件、成立／不成立后的目标步骤、结束本轮、固定次数重复和向后跳转循环。条件复用现有沙箱，条件内修改不会应用；不成立时跳过整个请求。参见[控制顺序、限制与运行证据](docs/SCENARIO-CONTROL-COVERAGE.md)。

场景支持命名并行块和可配置并发数，成员可使用条件及重复。分支变量保持隔离，完成后汇合；不同写值会停止本块更新，历史使用本批次全部分支的私密值脱敏。参见[并行语义、证据与剩余范围](docs/SCENARIO-PARALLEL-COVERAGE.md)。

集合／场景运行后自动保存脱敏报告，记录来源版本、步骤结果、断言和耗时。可查看、筛选、删除报告、定位当前失败接口，并导出 JSON、CSV、JUnit XML 或独立的中英文 HTML。每个工作区保留最近 100 份报告，即时正文、响应头、日志和变量值不写入报告。参见[报告证据与剩余范围](docs/SAVED-RUN-REPORT-COVERAGE.md)。
