# 调研交付状态

2026-10-02：根据用户“先获取 Apifox 和 Postman 完整功能选项”的要求，暂停产品实现。

- Apifox 官方索引：759 个入口，逐 URL 校验无遗漏。
- Postman 官方 sitemap：1,248 个入口，逐 URL 校验无遗漏。
- 选定当前产品文档：1,034 篇，正文成功读取、未截断、SHA256 校验通过。
- CommonMark 章节：8,050 条，保留级别、父节点、原文行号；代码注释不计入。
- 功能目标草案：28 个模块、395 项，全部 pending；来源为领域参考，不假装逐项厂商支持证明。
- 具体配置：23 组已核实的重点菜单/字段/枚举与套餐、平台限制。未登录竞品，未声称穷尽登录后或企业专属 UI。
- 独立审查：基线问题已修正，复核无严重/重要残留；见 research-review-fixes.md。
- 校验：3 个解析回归测试、目录/来源/章节层级/依赖 DAG/CSV/链接一致性通过；4 个 Actions 工作流通过 actionlint v1.7.10 静态检查。

仓库发布的是上述研究资料、生成脚本与 Actions 工作流。Rust/React/Tauri 骨架仍在本地且未验证；无服务端二进制、桌面安装包或 Docker 镜像发行。

Actions：Research integrity 实际校验资料；Application CI、Server/Desktop Builds、Docker Multi-platform 已配置源代码检测，应用源文件未发布时明确跳过构建。未来代码发布后仍需验证真实平台构建，不能据工作流存在宣称产品构建成功。

后续实施：以 FEATURE-MATRIX.md / CAPABILITY-MAP.md 拆分每个模块的需求、成熟库选型、兼容性用例与实施任务；替换调研前简化规格，保持独立服务端、独立桌面、托管同步、二进制内嵌前端和跨平台发行要求。
