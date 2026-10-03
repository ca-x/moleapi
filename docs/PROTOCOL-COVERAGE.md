# API 类型覆盖与 Rust 库复用

用户要求：覆盖 Apifox/Postman 当前支持的类型，优先使用现成 Rust crate，不能用普通 HTTP/菜单占位代替专用协议实现。公开产品证据来自2026-10-02已读取的官方目录/正文；crate 元数据于2026-10-03从 crates.io API 查询，另通过 agent-browser 阅读 docs.rs。仓库已推送实时协议提交 b1ffce7；下表区分已验证实现、GitHub Actions 分发与仍待实现的类型。

| 类型/专用客户端 | 官方产品证据 | MoleAPI 当前状态 | Rust 库方向 |
| --- | --- | --- | --- |
| HTTP/REST | 两者 |已验证并构建 | reqwest、url、现有公共网络策略 |
| SSE | 两者 |真实连接/事件/断开已跑通；b1ffce7 平台及 Docker Actions 已通过 |现用 eventsource-stream0.2.3、reqwest |
| WebSocket | 两者 |真实文本/二进制收发/消息/断开已跑通；b1ffce7 平台及 Docker Actions 已通过 |现用 reqwest-websocket0.5.1、tungstenite |
| GraphQL Query/Mutation/Introspection/Subscription | 两者 |专用 GraphiQL 客户端、Rust query/mutation、保存/恢复 schema、graphql-transport-ws subscription 已实现；真实服务与复审通过，当前变更平台 CI 待运行 | 现用 async-graphql-parser7.2.1、cynic-introspection3.14.0、graphql-ws-client0.13.0、apollo-parser0.8.6 |
| gRPC unary/三种 streaming/Reflection/proto | 两者 |专用客户端、四种实际调用、proto/Reflection、metadata/status/trailers、半关闭/取消已实现并复审；平台分发验证按用户要求留到功能整合阶段 |现用 tonic0.14.6、prost0.14.4、prost-reflect0.16.5、protox0.9.1、tonic-reflection0.14.6 |
| Socket.IO/Engine.IO/namespace/event/ack | 两者 |待实现；原生WebSocket不算Socket.IO | rust_socketio0.6.0、rust_engineio0.6.0 |
| MQTT publish/subscribe/QoS/TLS | Postman，Apifox公开目录未定位 |待实现 | rumqttc0.25.1 |
| SOAP/XML/WSDL | 两者 |HTTP可手工发送XML；专用SOAP/WSDL尚未实现 | wsdl0.1.3用于读取WSDL、quick-xml0.42.0；rsoap0.4.0为编译时生成客户端，不能直接当运行时导入 |
| MCP调试/客户端/服务端配置 | 两者 |待实现 |官方RustSDK rmcp3.5.0 |
| A2A/Agent Card/任务与流 | Apifox，Postman专用A2A公开目录未定位 |待实现 | a2a-sdk0.7.0；必须检查其客户端/传输能力，a2a-protocol这一猜测名称返回404，不采用 |
| TCP Socket/报文处理 | Apifox，Postman公开目录未定位 |待实现 | tokio、tokio-util0.7.19现成 codec、TLS库 |
| Dubbo | Apifox，Postman公开目录未定位 |待实现 |Apache dubbo0.4.0；确认Triple与经典Dubbo/Hessian覆盖，hessian2仅是序列化库，不能据此宣称完整Dubbo客户端 |
| Webhook监听/检查/重放 | 两者 |待实现专用入口 | Axum、现有HTTP客户端及成熟请求/解析库 |
| Data request：PG/MySQL、local/remote file SQL | Postman，Apifox公开目录未定位 |待实现；应用存储支持三种数据库不算Data客户端 |已有SQLx0.8.6优先复用；DataFusion/DuckDB候选需评估文件SQL、网络/文件访问限制和分发体积 |

不增加没有公开证据的“竞品已支持”结论；未定位不是断言不支持。HTTP传输承载SOAP/GraphQL/MCP/A2A等不等于完成它们的 schema、方法选择、事件、任务、鉴权和编辑器功能。Apifox/Postman类型与功能参考链接见[完整矩阵](FEATURE-MATRIX.md)，原始目录见[Apifox](APIFOX-CATALOG.md)/[Postman](POSTMAN-CATALOG.md)。

## 可验证交付顺序

先关闭SSE/WebSocket当前审查并运行平台/三数据库CI，然后GraphQL、gRPC、Socket.IO/MQTT、SOAP/WSDL、MCP/A2A、TCP/Dubbo、Webhook/Data。每个类型须实现保存/导入、实际执行、专用编辑器、结果/事件、超时/取消、鉴权/网络限制、真实fixture测试、桌面/服务端可用和平台构建；只是增加kind、依赖或按钮不算完成。按现有架构拆成独立crate/功能模块，保留原始schema和前向可识别的类型字段。

官方库负责协议语法、帧、编解码、schema等；应用代码只负责工作区/授权/生命周期/UI与库的接口。SDK若缺少共享网络策略所需的连接器注入，先核实成熟扩展点或最小依赖补丁，不能退化到未经检查的第二次DNS/重定向，也不能重写协议来绕过库。版本是候选/核实证据，不是必须升级现有依赖；SQLx现有0.8.6仍复用，避免为了新增Data功能升级整个ORM。

[crate API证据](references/rust-protocol-crates.json)保留包版本、MSRV、仓库与features。docs.rs已读取tonic0.14.6、dubbo0.4.0及wsdl0.1.3；wsdl明确是roxmltree上的WSDL接口，SOAP包名不能盲选，搜索命中的soap0.1.0实际上是神经模型。
