<p align="center"><img src="assets/brand/logo.png" width="160" alt="MoleAPI Logo"></p>
<h1 align="center">MoleAPI</h1>
<p align="center">可自托管的 API 工作台，配套独立离线桌面客户端。</p>

**当前处于开发阶段。基础工作台已实现，完整功能对齐仍在推进。** [实现与验证状态](docs/IMPLEMENTATION-STATUS.md)区分已验证行为、构建状态和后续模块。

服务端采用 Rust/Axum/SeaORM：SQLite 默认，PostgreSQL/MySQL 可配置。前端使用 React、Radix UI、TanStack Query、CodeMirror 和可调整面板；桌面端使用 Tauri，通过进程内 IPC 复用服务端 API，本地数据存于 SQLite，不需要另外启动服务端。

当前基础功能包括工作区/集合/请求编辑、环境变量、真实响应和断言、请求历史、响应示例与 Mock、集合测试、OpenAPI3.0/3.1 与 Postman2.1/cURL 导入导出、规范原文保留、可选自托管同步和明确的冲突处理。请求前/响应后脚本、分层变量和本地覆盖值已实现；SSE/WebSocket 支持实时连接、消息收发与断开，脚本使用独立 QuickJS 子进程及宿主超时保护。更多协议、完整协作与企业功能等按矩阵继续开发，不能据此宣称已完整替代竞品。

## 本地构建和运行

需要 Rust1.94+ 和 Node22+。运行服务端不需要 Node；前端及字体/编辑器资源已嵌入编译后的二进制。

```bash
npm --prefix web ci
npm --prefix web run build
cargo build --locked --release -p moleapi-server
./target/release/moleapi-server --bind 127.0.0.1:8787
```

打开 `http://127.0.0.1:8787`，使用首次启动日志中的初始化令牌创建账户。默认仅首次管理员可注册；额外注册用 `--allow-registration`。调试局域网接口可显式开启 `--allow-private-network`。

默认 SQLite 文件是 `./data/moleapi.db`，其他数据库通过 `--database-url` 或 `MOLEAPI_DATABASE_URL` 配置：

```bash
MOLEAPI_DATABASE_URL='postgresql://user:password@localhost:5432/moleapi' ./target/release/moleapi-server
MOLEAPI_DATABASE_URL='mysql://user:password@localhost:3306/moleapi' ./target/release/moleapi-server
```

数据库选择与备份要求见[数据库规格](docs/specs/database.md)。更换数据库 URL 不会自动搬迁已有数据。

## 桌面与 Docker

```bash
npm --prefix web run desktop:dev
npm --prefix web run desktop:build
docker compose pull
docker compose up --no-build -d
```

桌面构建需对应系统的 Tauri 开发依赖。Windows 配置 NSIS/MSI，macOS 配置 DMG，Linux 配置 deb/rpm/AppImage；安装包/镜像是否可用以实际 Actions 产物为准。生产签名与公证需要对应密钥，当前不宣称已签名。Docker 镜像由 GitHub Actions 的原生 amd64/arm64 runner 构建、运行验证并发布，本地直接拉取，无需构建。开发镜像为 `ghcr.io/ca-x/moleapi:feat-application`，可通过 `MOLEAPI_IMAGE` 选择已发布的版本。Docker 使用 `/data` 数据卷，也可配置外部 PostgreSQL/MySQL。

## 测试和自动构建

```bash
cargo test --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
npm --prefix web test
npm --prefix web run typecheck
```

GitHub Actions 分别检查应用、在 PostgreSQL16/MySQL8.4 实例上运行共享数据库用例、构建多平台服务端和 Tauri 安装包，以及 Docker amd64/arm64 镜像。

## 功能与设计依据

- [完整功能目标矩阵](docs/FEATURE-MATRIX.md)、[能力模块](docs/CAPABILITY-MAP.md)、[API 类型覆盖与 Rust 库](docs/PROTOCOL-COVERAGE.md)
- [Apifox 全部公开目录](docs/APIFOX-CATALOG.md)、[Postman 全部公开目录](docs/POSTMAN-CATALOG.md)
- [具体 UI/配置选项](docs/CONFIGURATION-OPTIONS.md)、[UI 分析](docs/UI-RESEARCH.md)
- [调研记录](docs/RESEARCH-NOTES.md)、[原始目录 JSON](docs/feature-options.json)、[功能目标 CSV](docs/features.csv)

透明 Logo 由项目发起者提供。竞品名称、截图与原始文档归各自权利人所有；保留来源用于调研，应用不使用竞品品牌资产。
