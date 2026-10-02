# MoleAPI 竞品调研记录

日期：2026-10-02。工具：agent-browser。目标是先取得 Apifox 和 Postman 的完整公开功能目录与 UI 依据，再设计 MoleAPI；当前 Rust/React/Tauri 骨架是未完成、未发布的草稿，不能据此宣称功能对齐。

## 已确认的来源与范围

- Apifox 官网：https://apifox.com/。实际用 browser open、snapshot、read 和 screenshot 查看。
- Apifox 完整公开索引：https://docs.apifox.com/llms.txt。原始索引列出 759 个入口，包括主文档、FAQ、最佳实践及重复分类入口。
- Apifox 页面布局：https://docs.apifox.com/page-layout。实际查看页面及官方工作台截图。
- Postman 官方全站 sitemap：https://learning.postman.com/sitemap.xml。列出 1,248 个入口，包括当前文档、v11 历史文档、Flows、帮助、开放 API 端点参考。
- Postman 当前导航：https://learning.postman.com/docs/getting-started/basics/navigating-postman/。实际浏览器导航显示 `Latest (v12)`。
- Postman 官方 HTTP 编辑器截图：https://assets.postman.com/postman-docs/v12/echo-request.png。
- Postman 官方工作台示意图：https://assets.postman.com/postman-docs/v12/workspace-diagram-02-28-26-v2.png。

`learning.postman.com/llms.txt` 在本次请求中返回官网文档首页 Markdown，不是全站索引。因此目录完整性依据是 sitemap，不能把这个响应当作完整 llms 索引。`www.postman.com/product/api-client/` 初次浏览超时，改用可访问的官方学习中心。未登录竞品账号；工作台 UI 依据为官方截图和正文，不宣称实际操作过登录后的菜单，也不宣称有权访问企业专有界面。

## 采集与证据

`references/apifox-catalog.json` 与 `references/postman-catalog.json` 保存全部索引入口。`references/fetch-manifest.json` 记录每篇选定当前产品文档的读取状态、最终 URL、摘要 SHA256、字符数、正文标题和 CommonMark 章节层级/父节点/原文行号。代码块中的注释不作为章节；source_anchor 未实测时为 null，不编造远端锚点。失败、重定向、截断不会被标为已验证。初次正则解析遗漏了两条标题包含 IPv6 方括号的入口，已改用 CommonMark 解析并以原索引 URL 集合验证无遗漏。当前全量目录为 759 + 1,248 = 2,007 个入口。

完整原始正文保存在本地 `docs/references/raw/`，不纳入 git。仓库保留源链接、索引、采集清单和分析，便于核验；本文不会将目录条数当作功能数，也不会将目录外没有看到的功能写成“不支持”。

复现：

```bash
agent-browser --session moleapi-reference open https://apifox.com
agent-browser --session moleapi-reference snapshot -i -u
agent-browser --session moleapi-reference read https://docs.apifox.com/llms.txt --raw
agent-browser --session moleapi-postman-index read https://learning.postman.com/sitemap.xml --raw
python3 -m pip install -r scripts/research-requirements.txt
python3 scripts/parse_apifox_catalog.py
python3 scripts/research_catalog.py --workers 12 --all
python3 scripts/build_feature_inventory.py
python3 scripts/build_parity_matrix.py
python3 scripts/build_configuration_options.py
python3 -m unittest discover -s scripts -p 'test_*.py'
python3 scripts/verify_research.py
```

## 对初始方案的修正

1. **不能把 Postman 兼容理解成只导入 2.1 JSON。** 当前 Collection schema 3.0.0 是请求、脚本、示例等组成的多文件 YAML 目录；旧的 2.1.0 仍需兼容。Newman 能运行导出的 2.1.0，不能直接运行 3.0。来源：https://learning.postman.com/docs/use/use-collections/collections-schemas/。
2. **变量不能只存一组字符串。** Apifox 有团队全局、项目全局、模块、环境、测试数据、临时六种作用域，优先级明确；本地值与远程值分开。来源：https://docs.apifox.com/global-environment-session-variables。
3. **原始规范与可执行请求需分开存储、相互关联。** Apifox 文档模式与调试模式分开，包含 Schema、响应组件、接口用例；Postman Spec Hub 包含规范、多文件、可视化编辑、校验与双向同步。导入不能把所有 schema 和引用扁平化后丢弃。来源：https://docs.apifox.com/design-and-request-mode；https://learning.postman.com/docs/design-apis/specifications/overview/。
4. **脚本与测试需要独立执行引擎。** 两者都有 JavaScript 前后置脚本、公共脚本/包和 pm API；固定的四种断言无法覆盖。兼容性需按 API 成员逐项测试，不能宣称“完全兼容”。来源：https://docs.apifox.com/postman-script-api；https://learning.postman.com/docs/tests-and-scripts/write-scripts/postman-sandbox-reference/overview/。
5. **流式协议需独立事件与会话模型。** SSE、WebSocket、gRPC streaming、MQTT、MCP、AI/A2A 不能全当作一次 HTTP JSON response。保存协议类型、帧/事件、连接状态与执行日志。
6. **测试场景不是顺序发送集合的别名。** Apifox 有条件、循环、分组、等待、跨场景引用、测试套件和数据驱动；Postman 还有 Flows、Datasets、Monitors、性能测试及本地模拟服务。
7. **同步需要覆盖协作资源。** 托管版不只是单用户 workspace 上传；项目成员、角色、分支、评论、历史、公开文档、运行报告要有独立模型。密钥本地值默认不参与团队同步。
8. **新功能不能遗漏。** 当前公开文档包含 Postman Native Git、Data client、API Catalog、Insights、Agent Mode Cloud、SDK/CLI Generator；Apifox 包含 AI Agent Debugger、A2A Debugger、MCP 和审计/密钥扫描。
9. **使用成熟 Rust 库。** OpenAPI 3.0 可评估 openapiv3，3.1 可评估 oas3（本次 docs.rs 验证为 0.22.0，只描述 3.1.x，不能声称单库自动覆盖 3.0）。JSON Schema 校验交给 jsonschema；其它协议与数据库复用成熟库。选择应以实际版本、许可、测试和平台能力验证为准。
10. **企业与供应商云功能保留在矩阵中。** SSO、SCIM、审计、BYOK、密钥扫描、服务目录、插件和云编排不能因为首版实现困难而从调研删去。供应商账单、公共 API 市场运营等在矩阵标明原性质，MoleAPI 的产品实现边界需在对应规格中明确。
