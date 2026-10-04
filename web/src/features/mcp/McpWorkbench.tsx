import { errorCopy } from "../../shared/i18n/errors";
import type { ErrorCopy } from "../../shared/i18n/errors";
import { t, useLanguage, translateCopy } from "../../shared/i18n";
import { useEffect, useRef, useState } from "react";
import { Badge, Button, Callout, Dialog, Flex, Tabs, Text, TextField } from "@radix-ui/themes";
import { Plus, RefreshCw, Send, Square, Trash2 } from "lucide-react";
import { Choice, Editor, Field, PairEditor, ToolButton } from "../../shared/ui";
import { id } from "../../shared/model";
import { useWorkbench } from "../workbench/context";
import SessionEventPane from "../protocols/SessionEventPane";
import type { ProtocolEvent } from "../protocols/types";
import type { McpCapability, McpCapabilities, McpConfig, McpMethod } from "./types";
import { callbackTemplate, object } from "./model";
import McpContent from "./McpContent";
import McpConfigDialog from "./McpConfigDialog";
const empty: McpCapabilities = { tools: [], resources: [], resource_templates: [], prompts: [] };
export default function McpWorkbench() {
  useLanguage();
  const state = useWorkbench();
  const latest = useRef(state); latest.current = state;
  const config = state.request?.protocol?.kind === "mcp" ? state.request.protocol : null;
  const channel = state.protocolSession;
  const { session, events, send, close, busy, sending, error, dropped } = channel;
  const identity = JSON.stringify([state.authenticated, state.accountId, state.draft?.id, state.request?.id, state.draft?.data.active_environment_id, session?.id]);
  const scope = useRef(identity); scope.current = identity;
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const [settingsOpen, setSettingsOpen] = useState(true);
  const [configOpen, setConfigOpen] = useState(false);
  const [tab, setTab] = useState("capabilities");
  const pane = useRef<HTMLElement>(null);
  function reveal(value: string) {
    setTab(value);
    if (pane.current) pane.current.scrollTop = 0;
  }
  useEffect(() => { if (pane.current) pane.current.scrollTop = 0; }, [tab, identity]);
  const [group, setGroup] = useState<keyof McpCapabilities>("tools");
  const [search, setSearch] = useState("");
  const [cache, setCache] = useState<{ scope: string; capabilities: McpCapabilities; info: Record<string, unknown> | null }>({ scope: "", capabilities: empty, info: null });
  const [pending, setPending] = useState<string[]>([]);
  const [answered, setAnswered] = useState<string[]>([]);
  const [reply, setReply] = useState<ProtocolEvent | null>(null);
  const [replySource, setReplySource] = useState("{}");
  const [replyError, setReplyError] = useState<ErrorCopy>("");
  const [replyBusy, setReplyBusy] = useState(false);
  const replyId = useRef<string | null>(null);
  replyId.current = reply?.message.kind === "mcp_callback" ? reply.message.callback_id : null;
  useEffect(() => { setPending([]); setAnswered([]); setReply(null); setReplyBusy(false); setReplyError(""); setConfigOpen(false); }, [identity]);
  useEffect(() => { setTab("capabilities"); setSearch(""); }, [state.accountId, state.draft?.id, state.request?.id]);
  const capabilitiesEvent = [...events].reverse().find(event => event.message.kind === "mcp_capabilities");
  const initialized = [...events].reverse().find(event => event.message.kind === "mcp_initialized");
  useEffect(() => {
    if (!capabilitiesEvent && !initialized) return;
    setCache(previous => ({
      scope: identity,
      capabilities: capabilitiesEvent?.message.kind === "mcp_capabilities" ? capabilitiesEvent.message : previous.scope === identity ? previous.capabilities : empty,
      info: initialized?.message.kind === "mcp_initialized" ? initialized.message.info : previous.scope === identity ? previous.info : null,
    }));
  }, [identity, capabilitiesEvent?.cursor, initialized?.cursor]);
  useEffect(() => {
    const completed = new Set(events.flatMap(event => event.message.kind === "mcp_result" || event.message.kind === "mcp_error" ? [event.message.request_id] : []));
    setPending(previous => previous.filter(value => !completed.has(value)));
  }, [events]);
  useEffect(() => {
    const completed = events.flatMap(event => {
      if (event.message.kind !== "mcp_notification" || event.message.method !== "moleapi/callback_completed") return [];
      const params = object(event.message.params);
      return typeof params?.callback_id === "string" ? [params.callback_id] : [];
    });
    if (!completed.length) return;
    setAnswered(previous => [...new Set([...previous, ...completed])]);
    if (replyId.current && completed.includes(replyId.current)) setReply(null);
  }, [events]);
  const capabilities = cache.scope === identity ? cache.capabilities : empty;
  const info = cache.scope === identity ? cache.info : null;
  const open = session?.state === "open";
  const active = !!session && ["connecting", "open"].includes(session.state);
  useEffect(() => { if (active) setSettingsOpen(false); }, [active]);
  function update(patch: Partial<McpConfig>) {
    const current = latest.current.request?.protocol;
    if (current?.kind === "mcp") latest.current.updateRequest({ protocol: { ...current, ...patch } });
  }
  async function run(method?: McpMethod) {
    const current = latest.current.request?.protocol;
    if (current?.kind !== "mcp" || !open || (!method && !(current.operation === "resources/read" ? current.uri : current.name))) return;
    const origin = identity;
    const ticket = id();
    setPending(values => [...values, ticket]);
    const sent = await send({ kind: "mcp_request", request_id: ticket, method: method || current.operation, name: current.name, uri: current.uri, arguments_source: current.arguments_source });
    if (mounted.current && scope.current === origin) {
      if (!sent) setPending(values => values.filter(value => value !== ticket));
      else if (!method || method !== "refresh") reveal("results");
    }
  }
  useEffect(() => {
    const dispatch = () => {
      if (open) void run(); else void latest.current.protocolSession.connect();
    };
    state.mcpRun.current = dispatch;
    return () => { if (latest.current.mcpRun.current === dispatch) latest.current.mcpRun.current = null; };
  }, [identity, open]);
  function inspectCallback(event: ProtocolEvent) {
    if (event.message.kind !== "mcp_callback") return;
    setReply(event); setReplySource(callbackTemplate(event.message.method)); setReplyError("");
  }
  async function answer(action: "send" | "decline" | "cancel") {
    if (reply?.message.kind !== "mcp_callback" || replyBusy || !open) return;
    const origin = identity, callback = reply.message.callback_id;
    setReplyBusy(true); setReplyError("");
    try {
      const result = action === "send" ? JSON.parse(replySource) : reply.message.method === "elicitation/create" ? { action: action === "decline" ? "decline" : "cancel" } : undefined;
      const sent = await send({ kind: "mcp_callback", callback_id: callback, ...(result !== undefined ? { result } : { error: { code: -32000, message: action === "decline" ? "Request declined" : "Request cancelled" } }) });
      if (sent && mounted.current && scope.current === origin && replyId.current === callback) { setAnswered(values => [...values, callback]); setReply(null); }
    } catch (caught) {
      if (mounted.current && scope.current === origin && replyId.current === callback) setReplyError(errorCopy(caught));
    } finally { if (mounted.current && scope.current === origin) setReplyBusy(false); }
  }
  function select(capability: McpCapability) {
    if (group === "tools") update({ operation: "tools/call", name: capability.name || "" });
    else if (group === "prompts") update({ operation: "prompts/get", name: capability.name || "" });
    else update({ operation: "resources/read", uri: capability.uri || capability.uriTemplate || "" });
  }
  const selected = config?.operation === "tools/call" ? capabilities.tools.find(value => value.name === config.name) : config?.operation === "prompts/get" ? capabilities.prompts.find(value => value.name === config.name) : [...capabilities.resources, ...capabilities.resource_templates].find(value => (value.uri || value.uriTemplate) === config?.uri);
  const callbacks = events.filter(event => event.message.kind === "mcp_callback" && !answered.includes(event.message.callback_id));
  const results = [...events].reverse().filter(event => event.message.kind === "mcp_result" || event.message.kind === "mcp_error");
  const [resultCursor, setResultCursor] = useState<number | null>(null);
  useEffect(() => { setResultCursor(null); }, [identity]);
  const result = results.find(value => value.cursor === resultCursor) || results[0];
  if (!config) return null;
  return <section ref={pane} className="mcp-workbench" aria-label={t("MCP 客户端")}>
    <div className="mcp-toolbar">
      <Flex gap="3" align="center" wrap="wrap">
        <Choice value={config.transport} label={t("MCP 传输")} disabled={active || busy} onChange={transport => update({ transport })} options={[{ value: "http", label: "Streamable HTTP" }, { value: "stdio", label: "STDIO" }]} />
        <Button size="1" variant="soft" color="gray" disabled={active || busy} onClick={() => setConfigOpen(true)}>{t("Host 配置")}</Button>
        {callbacks.length > 0 && <Button size="1" color="amber" variant="soft" onClick={() => inspectCallback(callbacks[0])}>{t("处理回调 ·")} {callbacks.length}</Button>}
        <Badge color={open ? "green" : "gray"}>{session?.state || t("未连接")}</Badge>
        {info && <Text size="1" color="gray">{t("协议")} {String(info.protocolVersion || info.protocol_version || t("已协商"))}</Text>}
        <Button size="1" variant="soft" color="gray" disabled={!open || sending} onClick={() => void run("refresh")}><RefreshCw size={14} />{t("刷新能力")}</Button>
        <Button size="1" variant="soft" color="gray" disabled={!active && !busy} onClick={() => void close()}><Square size={14} />{t("断开")}</Button>
      </Flex>
      <Text size="1" color="gray">{t("当前连接使用连接时的鉴权与环境变量；修改后请重新连接。")}</Text>
      {config.transport === "stdio" && <details className="mcp-stdio-details" open={settingsOpen} onToggle={event => setSettingsOpen(event.currentTarget.open)}>
        <summary>{t("STDIO 设置 ·")} <span className="mono">{config.command || t("选择可执行文件")}</span></summary>
        <div className="mcp-stdio-settings" tabIndex={0} role="region" aria-label={t("STDIO 连接设置")}>
        <Field label={t("MCP 可执行文件")}><TextField.Root value={config.command} disabled={active || busy} placeholder="/usr/bin/node" onChange={event => update({ command: event.target.value })} /></Field>
        <Text size="1" color="gray">{t("连接时启动此程序。服务端需在允许的可执行文件列表中配置它。")}</Text>
        <div className="mcp-arguments" role="group" aria-label={t("STDIO 参数")}>
          {config.args.map((value, index) => <Flex gap="2" key={index}>
            <TextField.Root aria-label={t("STDIO 参数 {{number}}", { number: index + 1 })} value={value} disabled={active || busy} onChange={event => update({ args: config.args.map((item, position) => position === index ? event.target.value : item) })} />
            <ToolButton label={t("移除 STDIO 参数 {{number}}", { number: index + 1 })} disabled={active || busy} onClick={() => update({ args: config.args.filter((_, position) => position !== index) })}><Trash2 size={14} /></ToolButton>
          </Flex>)}
          <Button size="1" variant="soft" color="gray" disabled={active || busy} onClick={() => update({ args: [...config.args, ""] })}><Plus size={14} />{t("添加参数")}</Button>
        </div>
        <PairEditor rows={config.env} disabled={active || busy} onChange={env => update({ env })} secrets keyLabel={t("STDIO 环境变量")} />
      </div></details>}
    </div>
    {(error || session?.reason) && <Callout.Root color={session?.state === "closed" ? "gray" : "red"} role="alert"><Callout.Text>{error || session?.reason}</Callout.Text></Callout.Root>}
    <Tabs.Root value={tab} onValueChange={reveal} className="mcp-tabs">
      <Tabs.List><Tabs.Trigger value="capabilities">{t("能力与请求")}</Tabs.Trigger><Tabs.Trigger value="results">{t("响应")}</Tabs.Trigger><Tabs.Trigger value="messages">{t("消息与回调")}</Tabs.Trigger><Tabs.Trigger value="server">{t("服务信息")}</Tabs.Trigger></Tabs.List>
      <Tabs.Content value="capabilities">
        <div className="mcp-capability-grid">
          <div className="mcp-capabilities">
            <Choice value={group} label={t("MCP 能力类型")} onChange={setGroup} options={[{ value: "tools", label: "Tools" }, { value: "resources", label: "Resources" }, { value: "resource_templates", label: "Resource Templates" }, { value: "prompts", label: "Prompts" }]} />
            <TextField.Root aria-label={t("搜索 MCP 能力")} value={search} placeholder={t("搜索名称、URI…")} onChange={event => setSearch(event.target.value)} />
            <div className="mcp-capability-list">
              {capabilities[group].filter(value => JSON.stringify(value).toLowerCase().includes(search.toLowerCase())).map((value, index) => <button type="button" key={String(value.name || value.uri || value.uriTemplate || index)} onClick={() => select(value)} aria-pressed={selected === value}>
                <span>{String(value.title || value.name || value.uri || value.uriTemplate || t("未命名能力"))}</span><small>{value.description || value.uri || value.uriTemplate}</small>
              </button>)}
            </div>
            {!capabilities[group].length && <Text size="1" color="gray">{t("连接后显示此类能力。服务器未提供时列表为空。")}</Text>}
          </div>
          <div className="mcp-request-editor">
            <Flex gap="3" wrap="wrap" align="end">
              <Choice value={config.operation} label={t("MCP 操作")} onChange={operation => update({ operation })} options={[{ value: "tools/call", label: t("调用工具") }, { value: "resources/read", label: t("读取资源") }, { value: "prompts/get", label: t("获取提示词") }]} />
              <Field label={config.operation === "resources/read" ? t("资源 URI") : t("MCP 方法名称")}><TextField.Root value={config.operation === "resources/read" ? config.uri : config.name} onChange={event => update(config.operation === "resources/read" ? { uri: event.target.value } : { name: event.target.value })} /></Field>
              <Button size="2" disabled={!open || sending || !(config.operation === "resources/read" ? config.uri : config.name)} onClick={() => void run()}><Send size={14} />{t("运行")}</Button>
            </Flex>
            {config.operation === "resources/read" && <Flex gap="2"><Button size="1" variant="soft" disabled={!open || sending || !config.uri} onClick={() => void run("resources/subscribe")}>{t("订阅资源")}</Button><Button size="1" variant="soft" color="gray" disabled={!open || sending || !config.uri} onClick={() => void run("resources/unsubscribe")}>{t("取消订阅")}</Button></Flex>}
            {config.operation !== "resources/read" && <Editor value={config.arguments_source} onChange={arguments_source => update({ arguments_source })} dark={state.dark} jsonMode height="240px" label={t("MCP JSON 参数")} />}
            {selected && <Editor value={JSON.stringify(selected, null, 2)} dark={state.dark} jsonMode readOnly height="220px" label={t("MCP 能力定义")} />}
          </div>
        </div>
      </Tabs.Content>
      <Tabs.Content value="results">
        {pending.length > 0 && <Flex gap="2" wrap="wrap" align="center"><Text size="1">{pending.length} {t("个请求处理中")}</Text>{pending.map(ticket => <Button key={ticket} size="1" variant="soft" color="gray" disabled={!open || sending} onClick={() => void send({ kind: "mcp_cancel", request_id: ticket })}><Square size={14} />{t("停止")} {ticket.slice(0,8)}</Button>)}</Flex>}
        <Flex gap="2" wrap="wrap">{results.map(event => <Button key={event.cursor} size="1" variant={result?.cursor === event.cursor ? "soft" : "ghost"} color="gray" onClick={() => setResultCursor(event.cursor)}>{event.message.kind === "mcp_result" || event.message.kind === "mcp_error" ? event.message.method : ""} · #{event.cursor}</Button>)}</Flex>
        {result?.message.kind === "mcp_result" ? <McpContent result={result.message.result} dark={state.dark} /> : result?.message.kind === "mcp_error" ? <Editor value={JSON.stringify(result.message.error, null, 2)} dark={state.dark} jsonMode readOnly height="240px" label={t("MCP 错误")} /> : <Text color="gray">{t("选择能力并运行，响应会显示在这里。")}</Text>}
      </Tabs.Content>
      <Tabs.Content value="messages"><SessionEventPane events={events} dropped={dropped} dark={state.dark} sessionId={session?.id} protocolLabel="MCP" onReply={inspectCallback} canReply={event => !!open && event.message.kind === "mcp_callback" && !answered.includes(event.message.callback_id)} /></Tabs.Content>
      <Tabs.Content value="server"><Editor value={JSON.stringify(info || {}, null, 2)} dark={state.dark} jsonMode readOnly height="100%" label={t("MCP 服务信息")} /></Tabs.Content>
    </Tabs.Root>
    <McpConfigDialog open={configOpen} onOpenChange={setConfigOpen} />
    <Dialog.Root open={!!reply} onOpenChange={value => { if (!value) setReply(null); }}><Dialog.Content maxWidth="840px"><Dialog.Title>{t("MCP 客户端回调")}</Dialog.Title><Dialog.Description>{t("检查服务器请求并手动填写响应。")}</Dialog.Description>
      <Editor value={JSON.stringify(reply?.message, null, 2)} dark={state.dark} jsonMode readOnly height="180px" label={t("MCP 回调请求")} />
      <Editor value={replySource} onChange={setReplySource} readOnly={replyBusy} dark={state.dark} jsonMode height="240px" label={t("MCP 回调响应")} />
      {replyError && <Text color="red" role="alert">{translateCopy(replyError)}</Text>}
      <Flex justify="end" gap="3" mt="4"><Button variant="soft" color="gray" disabled={replyBusy || !open} onClick={() => void answer("decline")}>{t("拒绝请求")}</Button><Button variant="soft" color="gray" disabled={replyBusy || !open} onClick={() => void answer("cancel")}>{t("取消请求")}</Button><Button loading={replyBusy} disabled={!open} onClick={() => void answer("send")}>{t("发送响应")}</Button></Flex>
    </Dialog.Content></Dialog.Root>
  </section>;
}
