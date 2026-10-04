# API 类型覆盖与 Rust 库复用

用户要求：覆盖 Apifox/Postman 当前支持的类型，优先使用现成 Rust crate，不能用普通 HTTP/菜单占位代替专用协议实现。公开产品证据来自2026-10-02已读取的官方目录/正文；crate 元数据于2026-10-03从 crates.io API 查询，另通过 agent-browser 阅读 docs.rs。仓库已推送实时协议提交 b1ffce7；下表区分已验证实现、GitHub Actions 分发与仍待实现的类型。

| 类型/专用客户端 | 官方产品证据 | MoleAPI 当前状态 | Rust 库方向 |
| --- | --- | --- | --- |
| HTTP/REST | 两者 |已验证并构建 | reqwest、url、现有公共网络策略 |
| SSE | 两者 |真实连接/事件/断开已跑通；b1ffce7 平台及 Docker Actions 已通过 |现用 eventsource-stream0.2.3、reqwest |
| WebSocket | 两者 |真实文本/二进制收发/消息/断开已跑通；b1ffce7 平台及 Docker Actions 已通过 |现用 reqwest-websocket0.5.1、tungstenite |
| GraphQL Query/Mutation/Introspection/Subscription | 两者 |专用 GraphiQL 客户端、Rust query/mutation、保存/恢复 schema、graphql-transport-ws subscription 已实现；真实服务与复审通过，当前变更平台 CI 待运行 | 现用 async-graphql-parser7.2.1、cynic-introspection3.14.0、graphql-ws-client0.13.0、apollo-parser0.8.6 |
| gRPC unary/三种 streaming/Reflection/proto | 两者 |专用客户端、四种实际调用、proto/Reflection、metadata/status/trailers、半关闭/取消已实现并复审；平台分发验证按用户要求留到功能整合阶段 |现用 tonic0.14.6、prost0.14.4、prost-reflect0.16.5、protox0.9.1、tonic-reflection0.14.6 |
| Socket.IO/Engine.IO/namespace/event/ack | 两者 |实际 namespace/path/Auth/监听/JSON+混合二进制/双向ACK/取消已实现并复审；WebSocket transport，Engine.IO4 / Socket.IO wire5；分发验证留到功能整合阶段 |现用 rust_socketio0.6.0、rust_engineio0.6.0，最小上游SDK连接器/字节/ACK补丁 |
| MQTT publish/subscribe/QoS/TLS | Postman，Apifox公开目录未定位 |实际3.1.1/5、QoS0/1/2、retain/Will/属性、订阅/重连、TCP/TLS/WS/WSS、保存消息与遥测已实现并复审；分发验证留到功能整合阶段 |现用 rumqttc0.25.1，最小连接地址/额度/PUBREC SDK补丁；真实Mosquitto测试 |
| SOAP/XML/WSDL | 两者 |专用1.1/1.2客户端、WSDL1.1多文件导入/Service/Port/Operation/XML模板、实际HTTP调用/鉴权/脚本/XML/Fault已实现；真实Spyne服务验证，协议与XML私密值/工作量审查及回归验证通过；分发验证留到功能整合阶段 |现用wsdl0.1.3、roxmltree0.18、quick-xml0.38、xmltree0.11、xsd-parser1.5.2；运行时模型和XML写入均复用成熟库 |
| MCP调试/客户端/服务端配置 | 两者 |真实Streamable HTTP/STDIO客户端、工具/资源/模板/提示词、通知/进度/取消、sampling/elicitation/roots手动回调、Host配置导入导出已实现并复审；OAuth、Apps、服务端发布与更高协议版本仍待共享模块补齐 |官方RustSDK rmcp3.5.0，严格复用其协议解析与状态机；有来源/许可记录的输入额度与进程清理补丁 |
| A2A/Agent Card/任务与流 | Apifox，Postman专用A2A公开目录未定位 |0.3 JSONRPC及1.0 JSONRPC/HTTP+JSON真实客户端、原始Card导入/发现/显式端点选择、消息/任务/流/产物/继续任务/推送配置CRUD/取消已实现并复审；分发验证留到功能整合阶段 |a2a-client0.1/0.2实际注入客户端；a2a-types，a2a-rs0.10仅用于真实1.0 fixture；有许可和来源记录的输入预算/SDK语义补丁 |
| TCP Socket/报文处理 | Apifox，Postman公开目录未定位 |真实TCP/TLS、文本/Hex/Base64、原始块/行/32位大小端长度分帧、半关闭/读取/取消/空闲限制、原始草稿/安全导出已实现；实际fixture及浏览器验证，分发验证留到整合 |Tokio、tokio-util BytesCodec/LinesCodec/LengthDelimitedCodec、tokio-rustls/rustls、hex/base64；浏览器Hex显示复用@scure/base |
| Dubbo | Apifox，Postman公开目录未定位 |待实现 |Apache dubbo0.4.0；确认Triple与经典Dubbo/Hessian覆盖，hessian2仅是序列化库，不能据此宣称完整Dubbo客户端 |
| Webhook监听/检查/重放 | 两者 | 已实现持久接收器、私密检查/导出、编辑二进制重放及取消；独立客户端显式本地监听已有真实套接字测试，原生UI/平台验证待完成 | Axum/Hyper、SeaORM、现有checked reqwest、base64 |
| Data request：PG/MySQL、local/remote file SQL | Postman，Apifox公开目录未定位 |待实现；应用存储支持三种数据库不算Data客户端 |已有SQLx0.8.6优先复用；DataFusion/DuckDB候选需评估文件SQL、网络/文件访问限制和分发体积 |

不增加没有公开证据的“竞品已支持”结论；未定位不是断言不支持。HTTP传输承载SOAP/GraphQL/MCP/A2A等不等于完成它们的 schema、方法选择、事件、任务、鉴权和编辑器功能。Apifox/Postman类型与功能参考链接见[完整矩阵](FEATURE-MATRIX.md)，原始目录见[Apifox](APIFOX-CATALOG.md)/[Postman](POSTMAN-CATALOG.md)。

## 可验证交付顺序

先关闭SSE/WebSocket当前审查并运行平台/三数据库CI，然后GraphQL、gRPC、Socket.IO/MQTT、SOAP/WSDL、MCP/A2A、TCP/Dubbo、Webhook/Data。每个类型须实现保存/导入、实际执行、专用编辑器、结果/事件、超时/取消、鉴权/网络限制、真实fixture测试、桌面/服务端可用和平台构建；只是增加kind、依赖或按钮不算完成。按现有架构拆成独立crate/功能模块，保留原始schema和前向可识别的类型字段。

官方库负责协议语法、帧、编解码、schema等；应用代码只负责工作区/授权/生命周期/UI与库的接口。SDK若缺少共享网络策略所需的连接器注入，先核实成熟扩展点或最小依赖补丁，不能退化到未经检查的第二次DNS/重定向，也不能重写协议来绕过库。版本是候选/核实证据，不是必须升级现有依赖；SQLx现有0.8.6仍复用，避免为了新增Data功能升级整个ORM。

[crate API证据](references/rust-protocol-crates.json)保留包版本、MSRV、仓库与features。docs.rs已读取tonic0.14.6、dubbo0.4.0及wsdl0.1.3；wsdl明确是roxmltree上的WSDL接口，SOAP包名不能盲选，搜索命中的soap0.1.0实际上是神经模型。

SOAP 当前边界：WSDL1.1，document/literal；RPC/encoded 与不能可靠生成模板的 XSD choice/derivation/attribute/wildcard 明确报错。允许原始 XML 编辑，但所选 binding、版本、Action 和操作 QName 仍检查。不会自动下载外部 import；需提供依赖文件。尚未实现 WSDL2、WS-Security、完整 XSD facet 校验或专用 XPath 提取。

MCP 当前边界：已验证协商协议2025-11-25；不把SDK版本号当成支持所有后续协议版本。HTTP不自动跟随重定向；STDIO需要绝对可执行路径，独立服务端需管理员配置允许路径，原生客户端显式连接才启动。JSONHTTP/单帧/STDIO行1MiB、流/标准输出8MiB，会话命令512、回调数/事件数等有明确上限。手動sampling不调用LLM、roots不自动读取文件；外部资源URI/HTML/SVG不会自动执行或获取。

A2A 当前边界：明确选择0.3或1.0，0.3仅JSONRPC，1.0支持JSONRPC与HTTP+JSON；未实现A2A gRPC绑定、OAuth交互调试或自动推送接收。仅显式操作注册推送，message配置中的隐式注册会被拒绝。任务取消是远端操作，本地Stop终止当前请求而不伪造任务取消。Card不自动选择地址、不下载file URI；原文保存与显式含私密值导出保留原始数据，默认导出屏蔽已知私密值。真实SDK测试9项、原文/私密值互换2项及浏览器实际调用/流/任务/本地Stop已验证。

TCP当前边界：只连接tcp://或tcps://显式端口；共享权限/预脚本/环境快照、检查并固定DNS地址。原始读取块不是对端报文边界；长度字段固定4字节，行编解码遵循成熟SDK。TLS实际线缆字节包括握手/记录开销。限制512发送命令、4096接收帧、20MiB输入/线缆、1MiB帧，实际配额/阻塞发送取消测试通过。已知非空私密值/私密发送会保守隐藏本次会话后续载荷；空私密变量不误隐藏。默认导出屏蔽敏感JSON/副本和不能可靠筛查的编码二进制草稿；显式完整导出保留原文。自定义分帧脚本/长度偏移、自动重连、mTLS管理、事件脚本、持久流历史仍待实现。
