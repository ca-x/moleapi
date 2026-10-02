<p align="center"><img src="assets/brand/logo.png" width="180" alt="MoleAPI Logo"></p>
<h1 align="center">MoleAPI</h1>
<p align="center">可自托管的 API 工作台，配套独立离线桌面客户端。</p>

**当前阶段：竞品调研与规格。应用尚未完成，没有可用发行包。**

目标是对齐 Apifox 与 Postman 的 API 设计、调试、Schema、环境、脚本、测试、Mock、文档、协议、协作等能力，并提供自托管同步与独立桌面使用。先完整获取公开功能选项，再分模块实现；不会把一个 HTTP 编辑器称为完整对齐。

- [调研交付状态与验证范围](docs/RESEARCH-STATUS.md)
- [完整功能对照与目标矩阵](docs/FEATURE-MATRIX.md)
- [Apifox 全部公开目录](docs/APIFOX-CATALOG.md)
- [Postman 全部公开目录](docs/POSTMAN-CATALOG.md)
- [原始目录和章节数据 JSON](docs/feature-options.json) / [目录 CSV](docs/feature-options.csv)
- [可筛选功能目标 CSV](docs/features.csv)
- [调研证据、版本差异与架构修正](docs/RESEARCH-NOTES.md)
- [具体 UI/字段/枚举与套餐平台条件](docs/CONFIGURATION-OPTIONS.md)
- [UI 观察与组件/动效约束](docs/UI-RESEARCH.md)
- [能力模块与依赖](docs/CAPABILITY-MAP.md)
- [服务端、桌面和 Docker 自动构建目标](docs/BUILD-AND-RELEASE.md)

技术方向：Rust、Axum、SQLite；React + TypeScript、Radix UI、TanStack Query、CodeMirror；Tauri 桌面端。OpenAPI、JSON Schema、协议、数据库等优先使用成熟 Rust 库。服务端与客户端均嵌入前端资源，桌面不依赖独立服务端。

透明 Logo 由项目发起者提供。竞品页面、名称和截图归各自权利人所有；本仓库保存目录、来源和分析用于调研，不复制完整竞品文档。
