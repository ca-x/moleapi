#!/usr/bin/env python3
"""Curated capability map; every competitor reference remains traceable to official catalog."""
from pathlib import Path
from urllib.parse import urlparse
import collections,csv,json
ROOT=Path(__file__).resolve().parents[1]
DOCS=ROOT/'docs'
REF=DOCS/'references'
features=[]
modules=[]


def group(module,title,names,a=None,p=None,target='桌面 + 服务端',notes='',depends=()):
    if module not in [m['id'] for m in modules]:
        modules.append({'id':module,'title':title,'depends_on':list(depends)})
    for name in names.split(';'):
        if not name.strip(): continue
        n=sum(x['module']==module for x in features)+1
        features.append({'id':f'{module}-{n:03d}','module':module,'feature':name.strip(),'apifox_sources':[f'https://docs.apifox.com/{x}.md' for x in (a or '').split(';') if x], 'postman_sources':[('https://learning.postman.com/'+x if x.startswith(('flows/', 'api-docs/')) else 'https://learning.postman.com/docs/'+x) for x in (p or '').split(';') if x], 'target':target,'status':'pending','notes':notes})

# UI structure and resource management
A='page-layout';P='getting-started/basics/navigating-postman'
group('workbench','工作台与资源导航','工作区/项目切换;多项目窗口;目录树与多级文件夹;资源搜索和过滤;请求标签页与固定标签;关闭当前/其它/全部标签;环境选择器与变量面板;可折叠左右面板;底部状态与执行控制台;多资源批量移动/复制/删除;未保存修改提示;请求/规范/模型/Markdown 类型标签',A,P)
group('workbench','工作台与资源导航','Light/Dark/System 外观;键盘快捷键和命令面板;语言设置;客户端网络设置;通知入口;Cookie 管理器;回收站与资源恢复','keyboard-shortcuts;language-settings;account-settings;page-layout','getting-started/installation/settings/shortcut-settings;getting-started/installation/settings/settings-overview;getting-started/basics/navigating-postman')
group('workbench','工作台与资源导航','本地独立离线工作区;离线转在线迁移','offline-space','getting-started/basics/using-api-client',notes='Apifox 离线空间入口需核实；Postman lightweight client 功能不等同完整在线空间。')

# HTTP and auth
A='request-url-and-method;request-headers;request-params-and-body';P='use/send-requests/create-requests/request-basics;use/send-requests/create-requests/parameters;use/send-requests/create-requests/headers'
group('http','HTTP 请求与响应','HTTP 方法和 URL;Path/Query 参数;Header 键值与继承;参数描述/启用状态/批量编辑;multipart/form-data 与文件上传;x-www-form-urlencoded;JSON Body;XML Body;Raw Text Body;Binary Body;GraphQL HTTP Body;URL 编解码',A,P,depends=('workbench',))
group('http','HTTP 请求与响应','MessagePack Body','request-params-and-body',None)
group('http','HTTP 请求与响应','状态/耗时/大小展示;JSON/XML 格式化与原文;响应内搜索和折叠;响应图像/HTML/可视化;响应 Header 与 Cookie;实际请求与连接信息;保存响应为示例;下载与复制响应;发送/取消/重发;请求历史与从历史恢复','api-response;actual-request;request-history;extract-response-example;response-data-visualization','use/send-requests/response-data/responses;use/send-requests/response-data/visualizer;use/send-requests/response-data/examples;use/send-requests/response-data/cookies;use/send-requests/response-data/troubleshooting-api-requests')
group('auth','请求鉴权','No Auth;从请求/目录/集合继承鉴权;API Key Header/Query;Bearer Token;Basic Auth;JWT 生成/算法/签名;Digest Auth;OAuth 1.0;OAuth 2.0 与 PKCE;OAuth Token 获取/刷新/管理;Hawk;NTLM;Akamai EdgeGrid','authorization-types;oauth2','use/send-requests/authorization/authorization-types;use/send-requests/authorization/oauth-20',depends=('http',))
group('auth','请求鉴权','Kerberos','kerberos',None)
group('auth','请求鉴权','AWS Signature',None,'use/send-requests/authorization/aws-signature')
group('auth','请求鉴权','Atlassian Token',None,'use/send-requests/authorization/atlassian')
group('auth','请求鉴权','公共 API Guided Auth',None,'use/send-requests/authorization/authentication-for-public-apis')
group('network','网络与传输设置','CA 与客户端证书/mTLS;TLS 验证设置;客户端网络代理;Web 端请求代理/Agent;共享文档跨域请求代理;Cookie 持久化/域与路径;重定向/超时/编码设置;HTTP/2','ca-and-client-certificates;network-proxy;client-side-request-proxy;request-proxy-in-web;share-document-proxy;http2;create-and-send-cookie','use/send-requests/authorization/certificates;getting-started/basics/about-postman-agent;use/send-requests/create-requests/request-settings;use/send-requests/response-data/cookies',depends=('http','auth'))
group('network','网络与传输设置','HTTP/HTTPS 流量捕获;代理捕获保存为请求;浏览器 Interceptor;Cookie 与浏览器同步',None,'use/capturing-request-data/capture-overview;use/capturing-request-data/capturing-https-traffic;use/capturing-request-data/interceptor;use/capturing-request-data/syncing-cookies')
group('network','网络与传输设置','Unix Domain Socket/Windows Named Pipe',None,'use/send-requests/protocols/uds-named-pipes/send-uds-named-pipes-requests',target='桌面/本地 Runner')

# Variables and secrets
A='global-environment-session-variables;environments-and-services';P='use/send-requests/variables/variables;use/send-requests/variables/managing-environments;use/send-requests/variables/team-environments'
group('variables','环境与变量','开发/测试/生产环境;环境前置 URL/服务配置;团队与项目全局变量;集合/模块变量;环境变量;测试数据变量;运行临时/局部变量;优先级解析与同名覆盖;本地值与共享值分离;变量引用和值/作用域预览;敏感变量显隐;环境导入导出/共享/权限;环境分支与合并;动态值/日期/随机数据',A,P,depends=('workbench',))
group('variables','环境与变量','对象/数组子属性引用;可视化响应提取 JSONPath/XPath/正则;变量提取写入作用域','global-environment-session-variables;extract-variables;dynamic-values','use/send-requests/variables/variables;tests-and-scripts/write-scripts/postman-sandbox-reference/pm-variables')
group('vault','密钥与凭证存储','密钥库引用而不是直接同步密钥;本地密钥库导入导出;外部密钥库连接;密钥访问与脚本授权控制','vault-secrets','use/postman-vault/postman-vault-secrets;use/postman-vault/postman-vault-integrations',depends=('variables',))
group('vault','密钥与凭证存储','1Password',None,'use/postman-vault/1password')
group('vault','密钥与凭证存储','AWS Secrets Manager;Azure Key Vault;HashiCorp Vault','vault-secrets','use/postman-vault/aws-secrets-manager;use/postman-vault/azure-key-vault;use/postman-vault/hashicorp-vault',notes='Apifox 此功能限商业旗舰版；provider 与密钥元数据共享，密钥值加密保存在本地且不共享。')

# Specifications
A='create-an-api;data-schemas;advanced-data-types;response-components;security-schemes';P='design-apis/collections/overview;design-apis/specifications/overview;design-apis/specifications/spec-hub-visual-editor;design-apis/specifications/component-library'
group('specifications','API 设计与规范','文档定义与调试用例分开;可视化接口设计;请求/响应字段类型/必填/默认值/描述;枚举与约束;数组与嵌套对象;oneOf/anyOf/allOf;discriminator;可复用 Schema/组件库;公共参数/常用字段;可复用响应组件;鉴权组件/security schemes;多请求与响应示例;接口状态/负责人/扩展字段;API 唯一标识与业务目录;JSON 等生成 Schema;OpenAPI 规范源文件维护;OpenAPI JSON/YAML 导入导出;规范与请求集合双向同步;多文件规范与引用;规范语法/Schema 校验;规范实时文档预览',A,P,depends=('workbench',))
group('specifications','API 设计与规范','OpenAPI 2.0/Swagger;OpenAPI 3.0;OpenAPI 3.1;保留 x- 扩展与原始规范','import-openapi-swagger;apifox-openapi-swagger-extension','design-apis/specifications/overview;api-governance/api-definition/openapi2;api-governance/api-definition/openapi3',notes='各版本必须独立验收。oas3 0.22.0 文档只承诺 OpenAPI 3.1.x。')
group('specifications','API 设计与规范','AsyncAPI 规范;protobuf 2/3 规范;GraphQL SDL;Smithy 2.0 规范',None,'design-apis/specifications/overview')

# Protocol-specific clients
A='sse';P='use/send-requests/response-data/responses'
group('protocols','多协议客户端','SSE 连接与事件流/搜索/保存',A,P,depends=('http','auth','variables'))
group('protocols','多协议客户端','GraphQL schema introspection/浏览器;GraphQL Query/Mutation/变量;GraphQL 独立客户端','graphql','use/send-requests/protocols/graphql/graphql-overview;use/send-requests/protocols/graphql/graphql-client-interface')
group('protocols','多协议客户端','WebSocket 连接/消息/日志;Socket.IO 事件监听与发送','websocket;socketio','use/send-requests/protocols/websocket/websocket-overview;use/send-requests/protocols/websocket/create-a-socketio-request')
group('protocols','多协议客户端','gRPC protobuf/service reflection;gRPC unary;gRPC client/server/bidirectional streaming;gRPC metadata/auth/TLS;gRPC 示例/Mock/脚本','grpc','use/send-requests/protocols/grpc/grpc-client-overview;use/send-requests/protocols/grpc/using-service-definition;use/send-requests/protocols/grpc/using-grpc-mock;use/send-requests/protocols/grpc/scripting-in-grpc-request')
group('protocols','多协议客户端','SOAP/XML/WSDL','soap;import-wsdl','use/send-requests/protocols/soap/making-soap-requests')
group('protocols','多协议客户端','MQTT broker/订阅/发布/QoS',None,'use/send-requests/protocols/mqtt-client/mqtt-client-overview')
group('protocols','多协议客户端','TCP Socket/报文处理/粘包拆包','tcp-socket;message-data-processor',None)
group('protocols','多协议客户端','Dubbo 连接/调用/文档','create-dubbo-api;debug-dubbo-api;dubbo-api-documentation',None)
group('protocols','多协议客户端','Webhook 监听/检查/重放','webhook','use/send-requests/protocols/webhooks',notes='公网接收地址由自托管服务端提供，离线桌面只能提供本地监听。')
group('protocols','多协议客户端','数据库 Data client 与 schema explorer',None,'use/send-requests/protocols/data/data-overview')

# Script runtime and pm compatibility
A='pre-post-processors;scripts;postman-script-api;common-scripts';P='tests-and-scripts/write-scripts/intro-to-scripts;tests-and-scripts/write-scripts/postman-sandbox-reference/overview;tests-and-scripts/write-scripts/packages/overview'
group('scripts','脚本与前后置操作','JavaScript 前置脚本;JavaScript 后置脚本;请求/目录/集合级脚本继承与顺序;公共脚本/包库;第三方 JS 类库;pm.request 修改;pm.response 读取;pm.variables/environment/globals/collectionVariables;pm.sendRequest 与跨请求数据;pm.test/expect 断言;Cookie API;Vault 脚本访问;脚本日志与调试;响应数据可视化;运行沙箱/限时/权限',A,P,depends=('http','variables','vault'))
group('scripts','脚本与前后置操作','数据库前后置步骤;MySQL/MongoDB/Redis/Oracle 连接;SQL 查询/结果提取','database;mysql;mongodb;redis;oracle',None,target='桌面/授权服务端 Runner')
group('scripts','脚本与前后置操作','调用外部程序/Java/Python/PHP/Shell 等','call-external-programs',None,target='桌面/授权服务端 Runner',notes='单二进制不能内置所有外部语言运行时；需声明可选依赖，不应伪装为已内置。')

# Mocking
A='mock;smart-mock;custom-mock;mock-priority-rules;mock-scripts;runner-mock';P='design-apis/mock-apis/overview;design-apis/mock-apis/local-mock-servers;design-apis/mock-apis/set-up-mock-servers;design-apis/mock-apis/matching-algorithm;design-apis/mock-apis/create-dynamic-responses'
group('mock','Mock 与模拟服务','响应示例 Mock;Schema 驱动生成;字段名/规则/枚举数据生成;请求 URL/参数/Header/Body 匹配;响应选择与规则优先级;动态响应/脚本;状态码/Header/延迟;Mock 调用日志;本地 Mock 服务;自托管 Mock 服务;私有/公开 Mock 权限;数据驱动 Mock',A,P,depends=('specifications','scripts'))
group('mock','Mock 与模拟服务','模拟场景/失败/延迟配置与日志',None,'design-apis/simulate-conditions')

# Tests and flows
A='assertions;validate-response;new-test-scenario;flow-control-conditions;pass-data-between-test-steps;data-driven-testing';P='tests-and-scripts/test-apis/test-apis;tests-and-scripts/running-collections/intro-to-collection-runs;tests-and-scripts/running-collections/building-workflows;tests-and-scripts/datasets/overview'
group('testing','自动化测试与数据集','单接口测试用例;响应 schema 自动校验;可视化断言;JSONPath/XPath/文本/正则断言;状态/Header/耗时断言;场景/集合运行;步骤排序与分组;步骤间数据传递;条件分支;For/ForEach 循环;等待与重试;跨场景/跨项目复用;从接口定义同步测试步骤;CSV/JSON 数据驱动;共享测试数据/数据集;数据库实时数据源;测试套件;批量运行;本地/Runner 执行;运行报告/失败定位/历史;报告导出','assertions;validate-response;new-test-scenario;flow-control-conditions;data-driven-testing;test-reports;7927162m0',P,depends=('scripts','protocols','specifications'))
group('flows','可视化工作流','节点创建/连接/复制;子流程模块复用;条件/循环/等待节点;请求/响应/触发器节点;过滤/转换/SQL/FQL/TypeScript;图表/表格可视化输出;流程调试/快照/场景;流程部署为 API;定时/HTTP/MCP 触发;AI/Agent 节点;AI 评估节点;第三方 Connector 节点;分享/嵌入/Native Git',None,'flows/overview;flows/build-flows/overview;flows/reference/blocks/overview;flows/reference/flows-query-language/introduction-to-fql',depends=('testing',),notes='Apifox 的场景树已归入 testing 模块；此处列 Postman Flows 的独立能力，不声称 Apifox 有所有对应节点。')
group('performance','性能与压力测试','虚拟用户/并发;固定/递增/阶梯负载;运行时长与停止控制;随机/顺序测试数据分配;延迟与吞吐量图表;错误率/失败断言;测试报告与导出;CLI 性能测试','performance-testing','tests-and-scripts/test-apis/performance-testing;tests-and-scripts/performance-testing/testing-api-performance;postman-cli/postman-cli-run-performance-test',depends=('testing',))
group('monitoring','定时任务与监控','定时场景/集合任务;时区与调度;运行环境/数据集;周期监控与断言;结果/历史/通知;自托管 Runner/网络内执行;Runner 配置/扩缩容','scheduled-tasks;universal-runner;self-hosted-runner','monitoring-your-api/intro-monitors;monitoring-your-api/runners/overview;tests-and-scripts/running-collections/scheduling-collection-runs',target='服务端 + 桌面管理',depends=('testing',))

# Publishing
A='publish-documentation-site;quick-share;custom-domain;page-layout-settings;documentation-visibility-settings;api-version';P='publishing-your-api/api-documentation-overview;publishing-your-api/publishing-your-docs;publishing-your-api/custom-doc-domains;publishing-your-api/viewing-documentation'
group('documentation','文档与分享发布','Markdown 文档资源;自动生成 API 文档;在线调试文档;私有/公开/密码分享;文档站发布;发布版本与历史;自定义域名;文档主题/布局/Logo;目录/顶部导航/搜索;示例与代码片段;可见性与权限;SEO/分析统计;Run in client 按钮;分享链接/嵌入',A,P,target='桌面编辑/预览 + 服务端发布',depends=('specifications','mock','workbench'))
group('documentation','文档与分享发布','自定义 CSS/JS/HTML','custom-css-js-html',None,notes='用户自定义站点内容的隔离与许可需在实现规格中明确。')

# Collaboration/versioning
A='management-center;project-member-management;team-member-management;member-roles-and-permissions;team-collaboration;api-comments';P='collaborating-in-postman/collaborate-in-postman-overview;administration/roles-and-permissions;collaborating-in-postman/using-workspaces/overview'
group('collaboration','团队与托管同步','用户/团队/组织/项目;成员邀请/移除;项目/资源权限;管理员/编辑/查看角色;实时同步与在线状态;本地离线改动再同步;修订冲突与差异;评论/讨论;活动与通知;团队共享环境;资源转移/归档;公共/内部/伙伴工作区','management-center;project-member-management;team-member-management;member-roles-and-permissions;team-collaboration;api-comments',P,target='服务端 + 桌面同步客户端',depends=('workbench','variables'))
group('versioning','版本、分支与合并','创建/切换/管理分支;在分支编辑与测试;Diff 对比与合并冲突;合并审查/Pull Request;Fork/拉取上游;资源版本历史与恢复;API 发布版本标签;Git 连接/提交/拉取/推送;本地 Git 目录与云同步边界','sprint-branch;merge-sprint-branch;api-history;api-version;7230090m0','collaborating-in-postman/using-version-control/version-control-overview;use/native-git/overview;use/native-git/develop-locally;use/native-git/collaborate',depends=('collaboration','specifications'))

# Interchange and CLI
A='import-and-export;manual-import;export-data;scheduled-import';P='getting-started/importing-and-exporting/importing-data;getting-started/importing-and-exporting/exporting-data'
group('interchange','导入导出与迁移','OpenAPI/Swagger JSON/YAML;Postman Collection 2.0/2.1 JSON;Postman Collection 3.0 多文件 YAML;环境/全局变量导入导出;cURL 导入;HAR 导入;文件/文件夹/URL 数据源;Git 数据源;定时导入/绑定规范源;导入合并/覆盖规则;规范/用例/示例/脚本保真;敏感值导出选择;数据备份与恢复',A,P,depends=('specifications','variables','scripts'))
group('interchange','导入导出与迁移','Insomnia;Markdown/apiDoc;Apipost/Eolink/Knife4j;NEI/docway/Apizza;WSDL','import-insomnia;import-markdown;import-apidoc;import-apipost;import-eolink;import-knife4j;import-nei;import-docway;import-apizza;import-wsdl',None)
group('cli','CLI、CI/CD 与开放 API','命令行登录与资源管理;本地集合/测试套件运行;环境/数据/过滤/失败退出码;JSON/JUnit/HTML 报告;GitHub Actions 集成;GitLab/Jenkins 等 CI 集成;Git 提交触发测试;开放 API/访问令牌;CLI schema/参数与命令发现;给编码 Agent 使用的 Skills','apifox-cli;cli-command-options;cicd;git-commit-triggered-testing;api-access-token;9212297m0','postman-cli/postman-cli-overview;postman-cli/postman-cli-options;postman-cli/postman-cli-reporters;postman-cli/postman-cli-github-actions;postman-cli/postman-cli-skills;api-docs/api-reference',depends=('testing','interchange'))
group('integrations','集成与扩展','Webhook/测试通知;Slack/Teams/PagerDuty 等通知;Git/CI 集成;IDE 文档生成;浏览器插件;自托管 Runner 与代理 Agent;插件/扩展能力','notification-targets;notification-events;self-hosted-runner;request-proxy-agent;apifox-idea-plugin','integrations/intro-integrations;integrations/installed-apps;integrations/webhooks;integrations/ci-integrations',depends=('cli','monitoring'))

# AI and MCP
A='apifox-ai;generate-test-cases;generate-data-schemas-with-ai;api-compliance-check;api-docs-completeness-check;field-naming';P='getting-started/basics/about-ai;use/agent-mode/overview;use/agent-mode/skills'
group('ai','AI 与 Agent 工作台','AI 生成/维护测试;AI 生成/修改 Schema;文档完整性/规范性检查;字段命名辅助;自然语言构建/调试请求;AI 请求与模型选择;本地/自托管模型接入;Prompt/会话管理;模型参数/认证;工具调用 trace 与耗时/token/成本;模型结果对比;AI 任务审核与可观测日志','apifox-ai;ai-agent-debugger',P,depends=('specifications','testing','protocols'))
group('ai','AI 与 Agent 工作台','AI Branch;A2A Agent Card/消息/附件/Metadata','ai-branch;a2a-debugger',None)
group('ai','AI 与 Agent 工作台','云端长任务/自动化/Sandbox;AI Context Graph;团队 Agent Skills/Recipes;AI Gateway/Passport',None,'use/agent-mode-cloud/overview;use/context-graph/get-started;use/agent-mode/skills;getting-started/basics/fabric-gateway;getting-started/basics/passport',target='可选服务端模块',notes='保留完整竞品选项；外部模型/API keys 由用户配置。')
group('mcp','MCP 客户端与服务端','MCP server 连接/工具/资源/提示词;MCP OAuth 调试;MCP 消息与工具调用检查;MCP 配置导入导出;将项目/公开文档/OpenAPI 暴露为 MCP;生成/发布 MCP server;MCP 在 AI/Flows 中调用','mcp;apifox-mcp-server;6327888m0;6327890m0;6327891m0','use/send-requests/protocols/mcp-requests/overview;use/send-requests/protocols/mcp-requests/oauth-debugger;use/send-requests/protocols/mcp-requests/export-mcp-server-config;postman-api-network/showcase/publish/mcp-servers/overview',depends=('protocols','specifications'))

# Enterprise and publishing extensions
A='organization-basic-operations;sso;scim-user-management;9254394m0;9252226m0';P='administration/admin-overview;administration/sso/intro-sso;administration/scim-provisioning/scim-provisioning-overview;administration/managing-your-team/audit-logs;administration/managing-your-team/secret-scanner/overview'
group('administration','组织管理与安全','组织/团队/用户组;SSO/SAML/OIDC 适配;SCIM 用户/组同步;账号/组映射与成员权限;审计日志;密钥扫描/泄漏发现;服务账号/API Token;企业安装部署策略;域验证/账号捕获;网络/IP 访问策略;安全管理与访问活动','organization-basic-operations;sso;scim-user-management;9254394m0;9252226m0',P,target='服务端 + 客户端访问',depends=('collaboration','vault'))
group('governance','API 治理与运行观察','API 规范 lint 与治理规则;自定义规则/函数;API Catalog/服务目录;发现服务/连接云与代码;scorecards/治理分组;运行与 CI 指标聚合;API 流量/错误/延迟分析;Kubernetes/云 Agent 接入;生产流量脱敏;组织/工作区/API 生命周期报告;密钥扫描/AI 使用报告','api-compliance-check','api-governance/api-governance-overview;api-catalog/overview;insights/overview;reports/reports-overview',target='可选服务端模块',depends=('administration','monitoring','specifications'))
group('generation','代码、SDK 与 CLI 生成','请求调用代码片段;OpenAPI 客户端/服务端代码;SDK 生成/类型与文档;SDK 多语言配置;SDK 自动发布/PR/自定义代码合并;CLI Generator;生成工具集','generate-code','use/send-requests/create-requests/generate-code-snippets;sdk-generator/overview;cli-generator/overview;design-apis/toolsets/overview',depends=('specifications',))
group('discovery','API 市场与供应商运营','API Hub/Public API Network;私有 API Network;API/集合模板与搜索;公开发布者与认证;API 使用/发布者分析;套餐/订单/账单/订阅','apihub;manage-subscriptions;7191758m0','postman-api-network/overview;collaborating-in-postman/private-api-network/overview;billing/buying',target='产品边界需在规格中明确',notes='记录竞品完整选项；公开全球市场与供应商收费后台不同于自托管工作台，不能默认冒充已有市场。',depends=('collaboration','documentation'))


group('data','数据请求与可复用数据集','交互式数据库/文件 Data client;SQL 编辑/补全/schema explorer;结果表排序/搜索/分页;CSV/JSON/Spreadsheet 数据源;本地文件/云存储数据源;PostgreSQL/MySQL/SQL Server;自定义 JDBC 数据源;多来源 SQL Views/关联查询;pm.datasets 脚本查询;数据集用于测试/Mock/Monitors;数据库 SSH 隧道;本地/桌面/浏览器功能边界',None,'use/send-requests/protocols/data/data-overview;tests-and-scripts/datasets/create-datasets;tests-and-scripts/datasets/example-dataset-views;tests-and-scripts/datasets/query-datasets-in-scripts',depends=('protocols','testing'),notes='Data client 与持久化 Dataset 是不同资源；SSH 隧道/本地文件的桌面与 Web 限制以各端实际能力验收。')
group('administration','组织管理与安全','数据加密与 BYOK',None,'administration/managing-your-team/byok-encryption',target='服务端 + 客户端访问')
group('administration','组织管理与安全','EU 数据地域与区域选择',None,'administration/enterprise/about-eu-data-residency',target='服务端部署 + 客户端连接配置',notes='Postman Enterprise EU 文档明确有功能例外；MoleAPI 对应为自选部署地域，不承诺复刻供应商全球数据中心。')
group('cli','CLI、CI/CD 与开放 API','Newman 2.1 集合 CLI 兼容;Newman 自定义 Reporter;Newman Docker/CI 集成;Newman/Runtime 嵌入模块 API;Collection SDK/Runtime/格式转换 API',None,'reference/newman-cli/command-line-integration-with-newman;reference/newman-cli/newman-custom-reporters;reference/newman-cli/newman-with-docker;reference/developer-resources/collection-sdk;reference/developer-resources/runtime-library',target='CLI/可选兼容适配器',notes='Newman 需要 Node runtime；不声称 Rust 二进制已内置 Newman。Collection 3.0 使用 Postman CLI，Newman 对应 2.1 导出。')
group('integrations','集成与扩展','VS Code 扩展/导入/调试/环境/脚本',None,'reference/vs-code-extension/overview',target='可选扩展')

# User-specific requirements have no competitor evidence obligation
for title,names,target in [
 ('构建与发行','服务端独立二进制;前端完全嵌入二进制;SQLite 默认部署/迁移/备份;Docker 镜像与持久化卷;Linux amd64/arm64 镜像;环境变量/配置文件/日志/健康检查','服务端'),
 ('桌面与发行','Tauri 独立离线客户端;本机数据库与本地凭据;安装包内嵌前端/字体/编辑器资源;Windows NSIS/MSI;macOS x64/arm64 DMG;Linux deb/rpm/AppImage;自动更新/签名策略;用户透明 Logo 与平台图标;GitHub Actions 服务端构建;GitHub Actions 桌面打包;GitHub Actions Docker/GHCR 发布;公开 ca-x/moleapi 仓库','发行'),
]: group('distribution',title,names,None,None,target=target,notes='用户明确需求；不以竞品文档证明。',depends=('workbench',))


def norm(url):
    p=urlparse(url);return p.netloc+p.path.rstrip('/').removesuffix('.md')


def render():
    catalogs=[]
    for name in ['apifox','postman']:catalogs+=json.loads((REF/f'{name}-catalog.json').read_text())
    indexed={norm(x['url']) for x in catalogs}
    manifest=json.loads((REF/'fetch-manifest.json').read_text())
    read={norm(x['url']) for x in manifest if x['verified']}
    availability={norm(x['url']):x.get('availability_evidence',[]) for x in manifest if x['verified']}
    unmatched=[]
    for f in features:
        for field in ['apifox_sources','postman_sources']:
            verified=[]
            for url in f[field]:
                if norm(url) in indexed: verified.append(url)
                else:unmatched.append({'feature_id':f['id'],'url':url})
            f[field]=verified
        f['evidence']={url:'read' if norm(url) in read else 'indexed' for url in f['apifox_sources']+f['postman_sources']}
        f['source_availability']={url:availability.get(norm(url),[]) for url in f['apifox_sources']+f['postman_sources']}
        f['source_mapping']='domain_reference_not_feature_support_assertion'
    # Missing guessed paths are removed, not manufactured as documentation evidence.
    (REF/'unmatched-matrix-sources.json').write_text(json.dumps(unmatched,ensure_ascii=False,indent=2))
    (DOCS/'features.json').write_text(json.dumps({'date':'2026-10-02','status':'research_specification','modules':modules,'features':features},ensure_ascii=False,indent=2))
    with (DOCS/'features.csv').open('w',newline='') as file:
        fields=['id','module','feature','target','status','apifox_sources','postman_sources','notes','source_availability']
        writer=csv.DictWriter(file,fieldnames=fields,extrasaction='ignore');writer.writeheader()
        writer.writerows([{**f,'apifox_sources':';'.join(f['apifox_sources']),'postman_sources':';'.join(f['postman_sources']),'source_availability':json.dumps(f['source_availability'],ensure_ascii=False)} for f in features])
    text=['# MoleAPI 完整功能对照与目标矩阵','','采集日期：2026-10-02。基于完整官方目录、已读取正文与官方界面截图。','','这份矩阵是待实施的功能目标，不是完成清单。所有条目均为 `pending`；当前代码骨架未验证，任何条目都不能被标记为“已对齐”。完整原始入口见 [Apifox 目录](APIFOX-CATALOG.md)、[Postman 目录](POSTMAN-CATALOG.md) 与 [可筛选 CSV](features.csv)。','','“正文/目录”指证据读取等级，不等于逐个控件已经实际操作。每个来源说明该功能域；细项要在对应模块规格中补充兼容性与验收条件。空来源表示本次未定位足够公开证据，不能推断竞品不支持。企业、Beta、云端功能以对应页面的版本/套餐为准。JSON/CSV 的 source_availability 保留正文中套餐/平台限制原句，source_mapping 明确是功能域参考，不是厂商逐项支持声明；具体下拉选项由公开章节层级与 UI 观察共同覆盖。','','| 模块 | 功能目标数 |','| --- | --- |']
    for module in modules:text.append(f'| {module["title"]} (`{module["id"]}`) | {sum(f["module"]==module["id"] for f in features)} |')
    text+=['',f'总计 {len(modules)} 个模块、{len(features)} 项功能目标。数量是本项目归并/拆分后的需求条目，不是官方宣称的功能数。','']
    for module in modules:
        text += [f'## {module["title"]}', '', '| ID | 功能选项 / 目标 | Apifox 参考 | Postman 参考 | MoleAPI 运行位置 |', '| --- | --- | --- | --- | --- |']
        fs=[f for f in features if f['module']==module['id']]
        for f in fs:
            sources=[]
            for field in ['apifox_sources','postman_sources']:
                links=[f'[{"正文" if f["evidence"][u]=="read" else "目录"}{i+1}]({u})' for i,u in enumerate(f[field])]
                sources.append(' '.join(links) or ('用户要求' if module['id']=='distribution' else '未定位（待核实）'))
            text.append(f'| {f["id"]} | {f["feature"]} | {sources[0]} | {sources[1]} | {f["target"]} |')
        notes=list(dict.fromkeys(f['notes'] for f in fs if f['notes']))
        if notes:text += ['', *['说明：'+n for n in notes]]
        text.append('')
    text += ['## 不能缩减掉的兼容性验收','','- Postman Collection 2.1 JSON 与 v12 Collection 3.0 多文件 YAML 都必须有回归样例，脚本/示例/作用域不能在导入后丢失。','- `pm.*` 兼容按成员与执行阶段逐项记录，不用“支持 JS”代替“兼容 Postman”。','- OpenAPI 2.0、3.0、3.1、JSON Schema dialect、$ref/多文件与 x- 扩展分别验收。规范原文必须可保留并可往返导出。','- HTTP/SSE/WebSocket/gRPC/MQTT/MCP/A2A 各自有执行模型、持久化资源、日志以及取消/超时语义。','- 团队共享值、本地覆盖值、Vault secret 引用分开；离线工作与服务器同步冲突需要明确行为与测试。','- 对标企业/云端能力时，用本地/自托管实现对应价值，不能把付费供应商 API 假装成不依赖外部服务的本地功能。','- Windows/macOS/Linux、服务端、独立桌面、自托管同步与 Docker 均有独立构建和验收路径。','']
    (DOCS/'FEATURE-MATRIX.md').write_text('\n'.join(text))
    cap=['# MoleAPI 能力模块与依赖','','状态：完整公开目录采集后的草拟能力边界；当前实现暂停，具体兼容性/平台行为仍需模块规格验证，初始 HTTP 骨架规格不再代表完整范围。','','| 稳定模块 ID | 职责 | 依赖 |','| --- | --- | --- |']
    cap += [f'| {m["id"]} | {m["title"]} | {", ".join(m["depends_on"]) or "—"} |' for m in modules]
    cap += ['','建议实施顺序：共享数据/工作台 → HTTP/鉴权/变量/网络 → 规范/脚本/协议 → 测试/Mock/文档 → 团队同步/版本 → CLI/Runner/工作流 → 企业/AI/治理/扩展。发行流水线在首个可构建版本起随各模块持续验证。','','每个模块单独编写需求、兼容清单与验收用例，保持可替换边界；不能把矩阵中较难的模块从范围中悄悄删除。模块规模决定分阶段实施，不表示后续模块已经获得测试或完成。','']
    (DOCS/'CAPABILITY-MAP.md').write_text('\n'.join(cap))
    print(json.dumps({'modules':len(modules),'features':len(features),'unmatched_source_candidates':len(unmatched)},ensure_ascii=False))

if __name__=='__main__':render()
