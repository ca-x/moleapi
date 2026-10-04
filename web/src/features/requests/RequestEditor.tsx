import ProtocolPane from "../protocols/ProtocolPane";
import RequestScripts from "../scripts/RequestScripts";
import ExamplesEditor from "./ExamplesEditor";
import AssertionsEditor from "../testing/AssertionsEditor";
import { ResponsePane } from "./ResponsePane";
import { useState, useEffect, lazy, Suspense } from "react";
import { Group, Panel, Separator } from "react-resizable-panels";
import {
  Badge,
  Button,
  Checkbox,
  Flex,
  Tabs,
  Text,
  TextArea,
  TextField,
} from "@radix-ui/themes";
import { Braces, Copy, FileText, Save, Send } from "lucide-react";
import { toast } from "sonner";
import { Choice, Editor, Field, PairEditor, ToolButton } from "../../shared/ui";
import { curlTemplate, id, safeMessage } from "../../shared/model";
import type { ApiResponse, RequestSpec } from "../../shared/types";

const GraphQLWorkbench = lazy(() => import("../graphql/GraphQLWorkbench"));
const GrpcWorkbench = lazy(() => import("../grpc/GrpcWorkbench"));
const SocketIoWorkbench = lazy(() => import("../socketio/SocketIoWorkbench"));
const MqttWorkbench = lazy(() => import("../mqtt/MqttWorkbench"));
const A2aWorkbench = lazy(() => import("../a2a/A2aWorkbench"));
import { a2aConfig } from "../a2a/model";
const McpWorkbench = lazy(() => import("../mcp/McpWorkbench"));
import { mcpConfig } from "../mcp/model";
const SoapWorkbench = lazy(() => import("../soap/SoapWorkbench"));
import { mqttConfig } from "../mqtt/model";
const methods = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];
const requestTabs = [
  { value: "query", label: "参数" },
  { value: "headers", label: "请求头" },
  { value: "body", label: "请求体" },
  { value: "auth", label: "鉴权" },
  { value: "assertions", label: "断言" },
  { value: "scripts", label: "脚本" },
  { value: "docs", label: "文档" },
  { value: "examples", label: "示例" },
  { value: "settings", label: "设置" },
];
export default function RequestEditor({
  request,
  protocolConnected = false,
  update,
  send,
  save,
  dirty,
  saving,
  busy,
  sending,
  dark,
  response,
  error,
}: {
  request: RequestSpec;
  protocolConnected?: boolean;
  update: (patch: Partial<RequestSpec>) => void;
  send: () => void;
  save: () => void;
  dirty: boolean;
  saving: boolean;
  busy: boolean;
  sending: boolean;
  dark: boolean;
  response: ApiResponse | null;
  error: string;
}) {
  const [tab, setTab] = useState("query");
  const kind = request.protocol?.kind || "http";
  const live = ["sse", "websocket"].includes(kind);
  const grpc = kind === "grpc";
  const socketio = kind === "socketio";
  const mqtt = kind === "mqtt";
  const soap = kind === "soap";
  const mcp = kind === "mcp";
  const a2a = kind === "a2a";
  const mcpStdio = request.protocol?.kind === "mcp" && request.protocol.transport === "stdio";
  const mqttWebSocket =
    mqtt && (/^wss?:/i.test(request.url) || request.url.includes("{{"));
  useEffect(() => {
    if (
      (live || grpc || socketio || mqtt || mcp || a2a) &&
      [
        "body",
        "assertions",
        "examples",
        ...(grpc || mcpStdio || (mqtt && !mqttWebSocket)
          ? ["query", ...(mqtt || mcpStdio ? ["headers", "auth"] : [])]
          : []),
      ].includes(tab)
    )
      setTab(mcpStdio ? "settings" : mqtt ? "auth" : grpc ? "headers" : "query");
  }, [live, grpc, socketio, mqtt, mcp, a2a, mcpStdio, mqttWebSocket, tab]);
  function changeProtocol(
    value:
      | "http"
      | "sse"
      | "websocket"
      | "graphql"
      | "grpc"
      | "socketio"
      | "mqtt"
      | "soap"
      | "mcp"
      | "a2a",
  ) {
    let url = request.url;
    try {
      const parsed = new URL(url);
      if (value === "mqtt") {
        if (parsed.protocol === "http:") parsed.protocol = "mqtt:";
        if (parsed.protocol === "https:") parsed.protocol = "mqtts:";
      } else if (value === "websocket" || value === "socketio") {
        if (parsed.protocol === "http:") parsed.protocol = "ws:";
        if (parsed.protocol === "https:") parsed.protocol = "wss:";
      } else {
        if (parsed.protocol === "ws:") parsed.protocol = "http:";
        if (parsed.protocol === "wss:") parsed.protocol = "https:";
      }
      url = parsed.toString();
    } catch {
      /* Templates are resolved by the selected environment. */
    }
    update({
      protocol:
        value === "a2a" ? a2aConfig() : value === "mcp" ? mcpConfig() : value === "soap"
          ? {
              kind: "soap",
              version: "1.1",
              service: "",
              port: "",
              operation: "",
              action: "",
            }
          : value === "mqtt"
            ? mqttConfig()
            : value === "socketio"
              ? {
                  kind: value,
                  namespace: "/",
                  path: "/socket.io/",
                  auth_source: "{}",
                  listeners: ["message"],
                  event: "message",
                  arguments_source: "[]",
                  attachments_base64: [],
                  request_ack: false,
                  ack_timeout_ms: 5000,
                }
              : value === "grpc"
                ? { kind: value, service: "", method: "", message_source: "{}" }
                : value === "graphql"
                  ? {
                      kind: value,
                      document: "",
                      variables: {},
                      connection_params: {},
                    }
                  : { kind: value },
      url,
      ...(value === "soap"
        ? { method: "POST", body_kind: "text" }
        : value === "graphql"
          ? { method: "POST" }
          : value !== "http"
            ? { method: "GET", body_kind: "none" }
            : {}),
    });
  }
  const copyCurl = async () => {
    try {
      await navigator.clipboard.writeText(curlTemplate(request));
      toast.success("已复制 cURL 模板，鉴权值以占位符表示");
    } catch {
      toast.error("剪贴板不可用");
    }
  };
  const formatBody = () => {
    try {
      update({ body: JSON.stringify(JSON.parse(request.body), null, 2) });
    } catch (error) {
      toast.error(safeMessage(error));
    }
  };
  return (
    <div className="request-workspace">
      <header className="request-heading">
        <Flex direction="column" gap="1">
          <Text size="1" color="gray">
            {kind === "http"
              ? "HTTP 请求"
              : a2a ? "A2A 客户端" : mcp ? "MCP 客户端" : grpc
                ? "gRPC 请求"
                : soap
                  ? "SOAP 请求"
                  : mqtt
                    ? "MQTT 会话"
                    : socketio
                      ? "Socket.IO 会话"
                      : kind === "graphql"
                        ? "GraphQL 请求"
                        : kind === "sse"
                          ? "SSE 事件流"
                          : "WebSocket 会话"}
          </Text>
          <TextField.Root
            className="request-title-input"
            variant="surface"
            aria-label="请求名称"
            value={request.name}
            onChange={(e) => update({ name: e.target.value })}
          />
        </Flex>
        <Flex gap="3" align="center">
          {dirty && (
            <Text size="1" color="gray">
              未保存
            </Text>
          )}
          <Button color="gray" variant="soft" loading={saving} onClick={save}>
            <Save size={15} />
            保存
          </Button>
          <ToolButton
            label={
              kind === "http" ? "复制 cURL 模板" : "此类型暂不提供 cURL 模板"
            }
            onClick={copyCurl}
            disabled={kind !== "http"}
          >
            <Copy size={17} />
          </ToolButton>
        </Flex>
      </header>
      <div className="url-toolbar">
        <Choice
          value={kind}
          onChange={changeProtocol}
          label="请求协议"
          options={[
            { value: "http", label: "HTTP" },
            { value: "sse", label: "SSE" },
            { value: "websocket", label: "WebSocket" },
            { value: "graphql", label: "GraphQL" },
            { value: "grpc", label: "gRPC" },
            { value: "socketio", label: "Socket.IO" },
            { value: "mqtt", label: "MQTT" },
            { value: "soap", label: "SOAP" },
            { value: "mcp", label: "MCP" },
            { value: "a2a", label: "A2A" },
          ]}
        />
        {a2a ? <Badge className="protocol-handshake" color="gray">A2A</Badge> : mcp ? <Badge className="protocol-handshake" color="gray">MCP</Badge> : mqtt ? (
          <Badge className="protocol-handshake" color="gray">
            MQTT Broker
          </Badge>
        ) : grpc ? (
          <Badge className="protocol-handshake" color="gray">
            HTTP/2 · gRPC
          </Badge>
        ) : live || socketio ? (
          <Badge className="protocol-handshake" color="gray">
            GET 握手
          </Badge>
        ) : (
          <Choice
            value={request.method}
            onChange={(method) => update({ method })}
            options={methods.map((value) => ({ value, label: value }))}
            label="HTTP 方法"
          />
        )}
        <TextField.Root
          className="url-input mono"
          size="3"
          aria-label="请求 URL"
          disabled={mcpStdio}
          value={mcpStdio ? "STDIO" : request.url}
          placeholder="https://api.example.com/v1/users"
          onChange={(e) => update({ url: e.target.value })}
        />
        <Button
          size="3"
          loading={busy}
          onClick={send}
          disabled={
            (request.protocol?.kind === "mcp" && mcpStdio ? !request.protocol.command : !request.url) ||
            sending ||
            ((live || grpc || socketio || mqtt) && protocolConnected) ||
            (request.protocol?.kind === "mcp" && protocolConnected && !(request.protocol.operation === "resources/read" ? request.protocol.uri : request.protocol.name))
          }
        >
          <Send size={16} />
          {a2a ? (protocolConnected ? "运行" : "连接") : mcp ? (protocolConnected ? "运行" : "加载能力") : grpc
            ? "调用"
            : !(live || socketio || mqtt)
              ? "发送"
              : protocolConnected
                ? "已连接"
                : "连接"}
        </Button>
      </div>
      <Group
        id="request-response-split"
        orientation="vertical"
        className="request-response-split"
        resizeTargetMinimumSize={{ fine: 12, coarse: 44 }}
      >
        <Panel
          id="request-options-panel"
          defaultSize={
            kind === "graphql" || grpc || socketio || mqtt || soap || mcp || a2a
              ? "20%"
              : "35%"
          }
          minSize={kind === "graphql" ? "10%" : "25%"}
          className="request-options-panel"
        >
          <Tabs.Root
            value={tab}
            onValueChange={setTab}
            className="request-tabs"
          >
            <Tabs.List>
              {requestTabs
                .filter((item) =>
                  kind === "graphql" || soap
                    ? item.value !== "body"
                    : grpc || mqtt || mcp || a2a
                      ? ![
                          ...(mqtt && !mqttWebSocket ? ["headers"] : []),
                          ...(mcpStdio ? ["headers", "auth"] : []),
                          ...(grpc || mcpStdio || (mqtt && !mqttWebSocket)
                            ? ["query"]
                            : []),
                          "body",
                          "assertions",
                          "examples",
                        ].includes(item.value)
                      : !(live || socketio || mqtt) ||
                        !["body", "assertions", "examples"].includes(
                          item.value,
                        ),
                )
                .map((item) => (
                  <Tabs.Trigger key={item.value} value={item.value}>
                    {grpc && item.value === "headers" ? "Metadata" : item.label}
                    {(item.value === "query"
                      ? request.query.length
                      : item.value === "headers"
                        ? request.headers.length
                        : item.value === "assertions"
                          ? request.assertions.length
                          : 0) > 0 && (
                      <span className="count">
                        {item.value === "query"
                          ? request.query.length
                          : item.value === "headers"
                            ? request.headers.length
                            : request.assertions.length}
                      </span>
                    )}
                  </Tabs.Trigger>
                ))}
            </Tabs.List>
            <Tabs.Content value="scripts">
              <RequestScripts request={request} update={update} dark={dark} />
            </Tabs.Content>
            <Tabs.Content value="query">
              <PairEditor
                rows={request.query}
                onChange={(query) => update({ query })}
                keyLabel="参数名"
                valueLabel="参数值"
              />
            </Tabs.Content>
            <Tabs.Content value="headers">
              <PairEditor
                rows={request.headers}
                onChange={(headers) => update({ headers })}
                keyLabel="Header"
                valueLabel="值"
              />
            </Tabs.Content>
            <Tabs.Content value="body">
              <div className="body-options">
                <Choice
                  value={request.body_kind}
                  onChange={(body_kind) => update({ body_kind })}
                  options={[
                    { value: "none", label: "无请求体" },
                    { value: "json", label: "JSON" },
                    { value: "text", label: "Raw Text" },
                    { value: "form", label: "x-www-form-urlencoded" },
                  ]}
                  label="请求体格式"
                />
                {request.body_kind === "json" && (
                  <Button
                    size="1"
                    variant="ghost"
                    color="gray"
                    onClick={formatBody}
                  >
                    <Braces size={14} />
                    格式化
                  </Button>
                )}
              </div>
              {request.body_kind !== "none" ? (
                <Editor
                  value={request.body}
                  onChange={(body) => update({ body })}
                  dark={dark}
                  jsonMode={request.body_kind === "json"}
                  height="210px"
                />
              ) : (
                <Text size="2" color="gray">
                  此请求不发送 Body。
                </Text>
              )}
            </Tabs.Content>
            <Tabs.Content value="auth">
              <div className="form-panel">
                <Field label="鉴权类型">
                  <Choice
                    value={request.auth.kind}
                    onChange={(kind) =>
                      update({ auth: { ...request.auth, kind } })
                    }
                    options={[
                      { value: "none", label: "No Auth" },
                      { value: "bearer", label: "Bearer Token" },
                      { value: "basic", label: "Basic Auth" },
                    ]}
                    label="鉴权类型"
                  />
                </Field>
                {request.auth.kind === "bearer" && (
                  <Field
                    label="Token"
                    hint="可使用 {{api_token}} 引用环境变量。"
                  >
                    <TextField.Root
                      type="password"
                      autoComplete="off"
                      value={request.auth.token}
                      onChange={(e) =>
                        update({
                          auth: { ...request.auth, token: e.target.value },
                        })
                      }
                    />
                  </Field>
                )}
                {request.auth.kind === "basic" && (
                  <>
                    <Field label="用户名">
                      <TextField.Root
                        value={request.auth.username}
                        onChange={(e) =>
                          update({
                            auth: { ...request.auth, username: e.target.value },
                          })
                        }
                      />
                    </Field>
                    <Field label="密码">
                      <TextField.Root
                        type="password"
                        autoComplete="off"
                        value={request.auth.password}
                        onChange={(e) =>
                          update({
                            auth: { ...request.auth, password: e.target.value },
                          })
                        }
                      />
                    </Field>
                  </>
                )}
              </div>
            </Tabs.Content>
            <Tabs.Content value="assertions">
              <AssertionsEditor request={request} update={update} />
            </Tabs.Content>
            <Tabs.Content value="docs">
              <div className="documentation-editor">
                <Field label="接口说明">
                  <TextArea
                    rows={6}
                    value={request.description}
                    onChange={(e) => update({ description: e.target.value })}
                    placeholder="接口用途、参数约束与调用注意事项…"
                  />
                </Field>
                <Flex align="center" gap="2">
                  <FileText size={16} />
                  <Text size="2" color="gray">
                    说明随请求一起保存和导出。
                  </Text>
                </Flex>
              </div>
            </Tabs.Content>
            <Tabs.Content value="examples">
              <ExamplesEditor request={request} update={update} dark={dark} />
            </Tabs.Content>
            <Tabs.Content value="settings">
              <div className="form-panel">
                <Field label="请求超时 (ms)" hint="最大 120000ms。">
                  <TextField.Root
                    type="number"
                    min="100"
                    max="120000"
                    value={request.timeout_ms}
                    onChange={(e) =>
                      update({ timeout_ms: Number(e.target.value) })
                    }
                  />
                </Field>
                <label className="checkbox-label">
                  <Checkbox
                    checked={request.follow_redirects}
                    onCheckedChange={(v) =>
                      update({ follow_redirects: v === true })
                    }
                  />
                  自动跟随重定向
                </label>
                <label className="checkbox-label">
                  <Checkbox
                    checked={request.verify_tls}
                    onCheckedChange={(v) => update({ verify_tls: v === true })}
                  />
                  验证 TLS 证书
                </label>
              </div>
            </Tabs.Content>
          </Tabs.Root>
        </Panel>
        <Separator
          className="request-response-separator"
          aria-label="调整请求与响应面板高度"
        >
          <span aria-hidden="true" />
        </Separator>
        <Panel
          id="request-response-panel"
          defaultSize={
            kind === "graphql" || grpc || socketio || mqtt || soap || mcp || a2a
              ? "80%"
              : "65%"
          }
          minSize="40%"
        >
          {a2a ? <Suspense fallback={<Text role="status">正在加载 A2A 客户端…</Text>}><A2aWorkbench /></Suspense> : mcp ? <Suspense fallback={<Text role="status">正在加载 MCP 客户端…</Text>}><McpWorkbench /></Suspense> : soap ? (
            <Suspense
              fallback={<Text role="status">正在加载 SOAP 客户端…</Text>}
            >
              <SoapWorkbench />
            </Suspense>
          ) : mqtt ? (
            <Suspense
              fallback={<Text role="status">正在加载 MQTT 客户端…</Text>}
            >
              <MqttWorkbench />
            </Suspense>
          ) : socketio ? (
            <Suspense
              fallback={<Text role="status">正在加载 Socket.IO 客户端…</Text>}
            >
              <SocketIoWorkbench />
            </Suspense>
          ) : grpc ? (
            <Suspense
              fallback={<Text role="status">正在加载 gRPC 客户端…</Text>}
            >
              <GrpcWorkbench />
            </Suspense>
          ) : kind === "graphql" ? (
            <Suspense
              fallback={<Text role="status">正在加载 GraphQL 编辑器…</Text>}
            >
              <GraphQLWorkbench />
            </Suspense>
          ) : live || socketio ? (
            <ProtocolPane />
          ) : (
            <ResponsePane
              fill
              response={response}
              error={error}
              dark={dark}
              busy={busy}
              onExample={() => {
                if (response) {
                  update({
                    examples: [
                      ...request.examples,
                      {
                        id: id(),
                        name: `${response.status} 示例`,
                        status: response.status,
                        headers: response.headers,
                        body: response.body,
                      },
                    ],
                  });
                  toast.success("示例已添加，保存后保留");
                }
              }}
            />
          )}
        </Panel>
      </Group>
    </div>
  );
}
