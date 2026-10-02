# MoleAPI 能力模块与依赖

状态：完整公开目录采集后的草拟能力边界；当前实现暂停，具体兼容性/平台行为仍需模块规格验证，初始 HTTP 骨架规格不再代表完整范围。

| 稳定模块 ID | 职责 | 依赖 |
| --- | --- | --- |
| workbench | 工作台与资源导航 | — |
| http | HTTP 请求与响应 | workbench |
| auth | 请求鉴权 | http |
| network | 网络与传输设置 | http, auth |
| variables | 环境与变量 | workbench |
| vault | 密钥与凭证存储 | variables |
| specifications | API 设计与规范 | workbench |
| protocols | 多协议客户端 | http, auth, variables |
| scripts | 脚本与前后置操作 | http, variables, vault |
| mock | Mock 与模拟服务 | specifications, scripts |
| testing | 自动化测试与数据集 | scripts, protocols, specifications |
| flows | 可视化工作流 | testing |
| performance | 性能与压力测试 | testing |
| monitoring | 定时任务与监控 | testing |
| documentation | 文档与分享发布 | specifications, mock, workbench |
| collaboration | 团队与托管同步 | workbench, variables |
| versioning | 版本、分支与合并 | collaboration, specifications |
| interchange | 导入导出与迁移 | specifications, variables, scripts |
| cli | CLI、CI/CD 与开放 API | testing, interchange |
| integrations | 集成与扩展 | cli, monitoring |
| ai | AI 与 Agent 工作台 | specifications, testing, protocols |
| mcp | MCP 客户端与服务端 | protocols, specifications |
| administration | 组织管理与安全 | collaboration, vault |
| governance | API 治理与运行观察 | administration, monitoring, specifications |
| generation | 代码、SDK 与 CLI 生成 | specifications |
| discovery | API 市场与供应商运营 | collaboration, documentation |
| data | 数据请求与可复用数据集 | protocols, testing |
| distribution | 构建与发行 | workbench |

建议实施顺序：共享数据/工作台 → HTTP/鉴权/变量/网络 → 规范/脚本/协议 → 测试/Mock/文档 → 团队同步/版本 → CLI/Runner/工作流 → 企业/AI/治理/扩展。发行流水线在首个可构建版本起随各模块持续验证。

每个模块单独编写需求、兼容清单与验收用例，保持可替换边界；不能把矩阵中较难的模块从范围中悄悄删除。模块规模决定分阶段实施，不表示后续模块已经获得测试或完成。
