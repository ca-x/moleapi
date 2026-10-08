import { liveError } from "./../../shared/i18n/errors";
import { t, useLanguage, liveTranslation } from "../../shared/i18n";
import CookieManager from "../cookies/CookieManager";
import ProtocolPane from "../protocols/ProtocolPane";
import { authenticationEligibility } from "../authentication/eligibility";
import { inheritedAuthSource } from "../authentication/inheritedSource";
import RequestAuthEditor from "../authentication/RequestAuthEditor";
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
import { curlTemplate, id } from "../../shared/model";
import type { ApiResponse, RequestSpec } from "../../shared/types";

import RequestBodyEditor from "../request-body/RequestBodyEditor";
import {bodyModePatch} from "../request-body/model";
const GraphQLWorkbench = lazy(() => import("../graphql/GraphQLWorkbench"));
const GrpcWorkbench = lazy(() => import("../grpc/GrpcWorkbench"));
const SocketIoWorkbench = lazy(() => import("../socketio/SocketIoWorkbench"));
const MqttWorkbench = lazy(() => import("../mqtt/MqttWorkbench"));
import { dataConfig } from "../data/model";
const DataWorkbench=lazy(()=>import("../data/DataWorkbench"));
import { tcpConfig } from "../tcp/model";
const TcpWorkbench = lazy(() => import("../tcp/TcpWorkbench"));
const GenerationDialog = lazy(() => import("../generation/GenerationDialog"));
const A2aWorkbench = lazy(() => import("../a2a/A2aWorkbench"));
import { a2aConfig } from "../a2a/model";
const McpWorkbench = lazy(() => import("../mcp/McpWorkbench"));
import { mcpConfig } from "../mcp/model";
const SoapWorkbench = lazy(() => import("../soap/SoapWorkbench"));
import { mqttConfig } from "../mqtt/model";
const methods = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];
const requestTabs = [
  { value: "query", get label() { return t("参数"); } },
  { value: "headers", get label() { return t("请求头"); } },
  { value: "body", get label() { return t("请求体"); } },
  { value: "auth", get label() { return t("鉴权"); } },
  { value: "assertions", get label() { return t("断言"); } },
  { value: "scripts", get label() { return t("脚本"); } },
  { value: "docs", get label() { return t("文档"); } },
  { value: "examples", get label() { return t("示例"); } },
  { value: "settings", get label() { return t("设置"); } },
];
export default function RequestEditor({
  request,
  workspaceData,
  cookieWorkspace,
  protocolConnected = false,
  bodyScope = "",
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
  cookieWorkspace?: string;
  workspaceData?: import("../../shared/types").WorkspaceData;
  protocolConnected?: boolean;
  bodyScope?: string;
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
  useLanguage();
  const [tab, setTab] = useState("query");
  const [generationOpen, setGenerationOpen] = useState(false);
  const kind = request.protocol?.kind || "http";
  const live = ["sse", "websocket"].includes(kind);
  const grpc = kind === "grpc";
  const socketio = kind === "socketio";
  const mqtt = kind === "mqtt";
  const soap = kind === "soap";
  const mcp = kind === "mcp";
  const data = kind === "data";
  const dataLocal=request.protocol?.kind==="data"&&request.protocol.source==="local_file";
  const tcp = kind === "tcp";
  const a2a = kind === "a2a";
  const mcpStdio = request.protocol?.kind === "mcp" && request.protocol.transport === "stdio";
  const mqttWebSocket =
    mqtt && (/^wss?:/i.test(request.url) || request.url.includes("{{"));
  useEffect(() => {
    if (
      (data || tcp || live || grpc || socketio || mqtt || mcp || a2a) &&
      [
        "body",
        "assertions",
        "examples",
        ...(tcp ? ["settings"] : []),
        ...(data || tcp || grpc || mcpStdio || (mqtt && !mqttWebSocket)
          ? ["query", ...(tcp || mqtt ? ["headers", "auth"] : mcpStdio ? ["headers"] : [])]
          : []),
      ].includes(tab)
    )
      setTab(data ? "auth" : tcp ? "scripts" : mcpStdio ? "settings" : mqtt ? "auth" : grpc ? "headers" : "query");
  }, [data, tcp, live, grpc, socketio, mqtt, mcp, a2a, mcpStdio, mqttWebSocket, tab]);
  function changeProtocol(
    value:
      | "data"
      | "tcp"
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
        value === "data" ? dataConfig() : value === "tcp" ? tcpConfig() : value === "a2a" ? a2aConfig() : value === "mcp" ? mcpConfig() : value === "soap"
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
      url: value === "data" ? "postgresql://localhost:5432/postgres" : url,
      ...(value === "tcp" ? {auth:{...request.auth,kind:"none"},headers:request.headers.map(row=>({...row,enabled:false})),query:request.query.map(row=>({...row,enabled:false}))} : {}),
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
      toast.success(liveTranslation("已复制 cURL 模板，鉴权值以占位符表示"));
    } catch {
      toast.error(liveTranslation("剪贴板不可用"));
    }
  };
  const formatBody = () => {
    try {
      update({ body: JSON.stringify(JSON.parse(request.body), null, 2) });
    } catch (error) {
      toast.error(liveError(error));
    }
  };
  return (
    <div className="request-workspace">
      {generationOpen && <Suspense fallback={null}><GenerationDialog open={generationOpen} onOpenChange={setGenerationOpen} /></Suspense>}
      <header className="request-heading">
        <Flex direction="column" gap="1">
          <Text size="1" color="gray">
            {kind === "http"
              ? t("HTTP 请求")
              : data ? t("Data 客户端") : tcp ? t("TCP 客户端") : a2a ? t("A2A 客户端") : mcp ? t("MCP 客户端") : grpc
                ? t("gRPC 请求")
                : soap
                  ? t("SOAP 请求")
                  : mqtt
                    ? t("MQTT 会话")
                    : socketio
                      ? t("Socket.IO 会话")
                      : kind === "graphql"
                        ? t("GraphQL 请求")
                        : kind === "sse"
                          ? t("SSE 事件流")
                          : t("WebSocket 会话")}
          </Text>
          <TextField.Root
            className="request-title-input"
            variant="surface"
            aria-label={t("请求名称")}
            value={request.name}
            onChange={(e) => update({ name: e.target.value })}
          />
        </Flex>
        <Flex gap="3" align="center">
          {dirty && (
            <Text size="1" color="gray"> {t("未保存")} </Text>
          )}
          <Button color="gray" variant="soft" loading={saving} onClick={save}>
            <Save size={15} /> {t("保存")} </Button>
          <Button variant="soft" color="gray" disabled={kind !== "http"} onClick={() => setGenerationOpen(true)}>{t("生成代码")}</Button>
          <ToolButton
            label={
              kind === "http" ? t("复制 cURL 模板") : t("此类型暂不提供 cURL 模板")
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
          label={t("请求协议")}
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
            { value: "tcp", label: "TCP" },
            { value: "data", label: "Data" },
            { value: "a2a", label: "A2A" },
          ]}
        />
        {data ? <Badge className="protocol-handshake" color="gray">Data</Badge> : tcp ? <Badge className="protocol-handshake" color="gray">TCP</Badge> : a2a ? <Badge className="protocol-handshake" color="gray">A2A</Badge> : mcp ? <Badge className="protocol-handshake" color="gray">MCP</Badge> : mqtt ? (
          <Badge className="protocol-handshake" color="gray">
            MQTT Broker
          </Badge>
        ) : grpc ? (
          <Badge className="protocol-handshake" color="gray">
            HTTP/2 · gRPC
          </Badge>
        ) : live || socketio ? (
          <Badge className="protocol-handshake" color="gray"> {t("GET 握手")} </Badge>
        ) : (
          <Choice
            value={request.method}
            onChange={(method) => update({ method })}
            options={methods.map((value) => ({ value, label: value }))}
            label={t("HTTP 方法")}
          />
        )}
        <TextField.Root
          className="url-input mono"
          size="3"
          aria-label={t("请求 URL")}
          disabled={mcpStdio || dataLocal || (data && protocolConnected)}
          value={mcpStdio ? "STDIO" : dataLocal && request.protocol?.kind==="data" ? request.protocol.file_name : request.url}
          placeholder="https://api.example.com/v1/users"
          onChange={(e) => update({ url: e.target.value })}
        />
        <Button
          size="3"
          loading={busy}
          onClick={send}
          disabled={
            (dataLocal ? false : request.protocol?.kind === "mcp" && mcpStdio ? !request.protocol.command : !request.url) ||
            sending ||
            ((data || tcp || live || grpc || socketio || mqtt) && protocolConnected) ||
            (request.protocol?.kind === "mcp" && protocolConnected && !(request.protocol.operation === "resources/read" ? request.protocol.uri : request.protocol.name))
          }
        >
          <Send size={16} />
          {a2a ? (protocolConnected ? t("运行") : t("连接")) : mcp ? (protocolConnected ? t("运行") : t("加载能力")) : grpc
            ? t("调用")
            : !(data || tcp || live || socketio || mqtt)
              ? t("发送")
              : protocolConnected
                ? t("已连接")
                : t("连接")}
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
            kind === "graphql" || data || tcp || grpc || socketio || mqtt || soap || mcp || a2a
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
                  data ? ["auth","settings","docs"].includes(item.value) : tcp ? ["scripts","docs"].includes(item.value) : kind === "graphql" || soap
                    ? item.value !== "body"
                    : tcp || grpc || mqtt || mcp || a2a
                      ? ![
                          ...(mqtt && !mqttWebSocket ? ["headers"] : []),
                          ...(tcp ? ["headers", "auth"] : mcpStdio ? ["headers"] : []),
                          ...(tcp || grpc || mcpStdio || (mqtt && !mqttWebSocket)
                            ? ["query"]
                            : []),
                          "body",
                          "assertions",
                          "examples",
                        ].includes(item.value)
                      : !(tcp || live || socketio || mqtt) ||
                        !["body", "assertions", "examples"].includes(
                          item.value,
                        ),
                )
                .map((item) => (
                  <Tabs.Trigger key={item.value} value={item.value} aria-label={grpc && item.value === "headers" ? "Metadata" : item.label}>
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
              {tcp&&<Text size="1" color="gray">{t("TCP 支持连接前脚本；响应／事件脚本尚不执行，请清空响应脚本后连接。")}</Text>}
              <RequestScripts request={request} update={update} dark={dark} />
            </Tabs.Content>
            <Tabs.Content value="query">
              <PairEditor
                rows={request.query}
                onChange={(query) => update({ query })}
                keyLabel={t("参数名")}
                valueLabel={t("参数值")}
              />
            </Tabs.Content>
            <Tabs.Content value="headers">
              <PairEditor
                rows={request.headers}
                onChange={(headers) => update({ headers })}
                keyLabel="Header"
                valueLabel={t("值")}
              />
            </Tabs.Content>
            <Tabs.Content value="body">
              <div className="body-options">
                <Choice
                  value={request.body_kind}
                  onChange={(body_kind) => update(bodyModePatch(request,body_kind))}
                  options={[
                    { value: "none", label: t("无请求体") },
                    { value: "json", label: "JSON" },
                    { value: "text", label: "Raw Text" },
                    { value: "form", label: "x-www-form-urlencoded" },
                    {value:"binary",label:t("二进制文件")},
                    {value:"multipart",label:"multipart/form-data"},
                  ]}
                  label={t("请求体格式")}
                />
                {request.body_kind === "json" && (
                  <Button
                    size="1"
                    variant="ghost"
                    color="gray"
                    onClick={formatBody}
                  >
                    <Braces size={14} /> {t("格式化")} </Button>
                )}
              </div>
              {["binary","multipart"].includes(request.body_kind)?<RequestBodyEditor key={`${bodyScope}/${request.id}/${request.body_kind}`} kind={request.body_kind} value={request.body} change={body=>update({body})} busy={busy||sending} dark={dark}/>:request.body_kind !== "none" ? (
                <Editor
                  value={request.body}
                  onChange={(body) => update({ body })}
                  dark={dark}
                  jsonMode={request.body_kind === "json"}
                  height="210px"
                />
              ) : (
                <Text size="2" color="gray"> {t("此请求不发送 Body。")} </Text>
              )}
            </Tabs.Content>
            <Tabs.Content value="auth">
              <RequestAuthEditor auth={request.auth} change={auth=>update({auth})} dark={dark} credentials={!mcpStdio} {...authenticationEligibility(request)}/>
              {request.auth.kind==="inherit" && workspaceData && <Text size="1" color="gray">{t("继承来源：{{value0}}",{value0:(()=>{const source=inheritedAuthSource(workspaceData,request);return source.scope==="collection"?source.name:source.scope==="workspace"?t("工作区鉴权"):source.scope==="dynamic"?t("执行时解析环境变量后确定"):"No Auth";})()})}</Text>}
            </Tabs.Content>
            <Tabs.Content value="assertions">
              <AssertionsEditor request={request} update={update} />
            </Tabs.Content>
            <Tabs.Content value="docs">
              <div className="documentation-editor">
                <Field label={t("接口说明")}>
                  <TextArea
                    rows={6}
                    value={request.description}
                    onChange={(e) => update({ description: e.target.value })}
                    placeholder={t("接口用途、参数约束与调用注意事项…")}
                  />
                </Field>
                <Flex align="center" gap="2">
                  <FileText size={16} />
                  <Text size="2" color="gray"> {t("说明随请求一起保存和导出。")} </Text>
                </Flex>
              </div>
            </Tabs.Content>
            <Tabs.Content value="examples">
              <ExamplesEditor request={request} update={update} dark={dark} />
            </Tabs.Content>
            <Tabs.Content value="settings">
              <div className="form-panel">
                {cookieWorkspace && <CookieManager key={bodyScope} workspace={cookieWorkspace} environment={workspaceData?.active_environment_id} url={request.url}/>}
                <Field label={t("请求超时 (ms)")} hint={t("最大 120000ms。")}>
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
                  /> {t("自动跟随重定向")} </label>
                <label className="checkbox-label">
                  <Checkbox
                    checked={request.verify_tls}
                    onCheckedChange={(v) => update({ verify_tls: v === true })}
                  /> {t("验证 TLS 证书")} </label>
              </div>
            </Tabs.Content>
          </Tabs.Root>
        </Panel>
        <Separator
          className="request-response-separator"
          aria-label={t("调整请求与响应面板高度")}
        >
          <span aria-hidden="true" />
        </Separator>
        <Panel
          id="request-response-panel"
          defaultSize={
            kind === "graphql" || data || tcp || grpc || socketio || mqtt || soap || mcp || a2a
              ? "80%"
              : "65%"
          }
          minSize="40%"
        >
          {data ? <Suspense fallback={<Text role="status">{t("正在加载 Data 客户端…")}</Text>}><DataWorkbench /></Suspense> : tcp ? <Suspense fallback={<Text role="status">{t("正在加载 TCP 客户端…")}</Text>}><TcpWorkbench /></Suspense> : a2a ? <Suspense fallback={<Text role="status">{t("正在加载 A2A 客户端…")}</Text>}><A2aWorkbench /></Suspense> : mcp ? <Suspense fallback={<Text role="status">{t("正在加载 MCP 客户端…")}</Text>}><McpWorkbench /></Suspense> : soap ? (
            <Suspense
              fallback={<Text role="status">{t("正在加载 SOAP 客户端…")}</Text>}
            >
              <SoapWorkbench />
            </Suspense>
          ) : mqtt ? (
            <Suspense
              fallback={<Text role="status">{t("正在加载 MQTT 客户端…")}</Text>}
            >
              <MqttWorkbench />
            </Suspense>
          ) : socketio ? (
            <Suspense
              fallback={<Text role="status">{t("正在加载 Socket.IO 客户端…")}</Text>}
            >
              <SocketIoWorkbench />
            </Suspense>
          ) : grpc ? (
            <Suspense
              fallback={<Text role="status">{t("正在加载 gRPC 客户端…")}</Text>}
            >
              <GrpcWorkbench />
            </Suspense>
          ) : kind === "graphql" ? (
            <Suspense
              fallback={<Text role="status">{t("正在加载 GraphQL 编辑器…")}</Text>}
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
                        name: t("{{value0}} 示例", { value0: response.status }),
                        status: response.status,
                        headers: response.headers,
                        body: response.body,
                      },
                    ],
                  });
                  toast.success(liveTranslation("示例已添加，保存后保留"));
                }
              }}
            />
          )}
        </Panel>
      </Group>
    </div>
  );
}
