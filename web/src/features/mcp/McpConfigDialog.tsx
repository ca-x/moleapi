import type { ErrorCopy } from "../../shared/i18n/errors";
import { LocalizedError, errorCopy } from "../../shared/i18n/errors";
import { t, useLanguage, message, translateCopy } from "../../shared/i18n";
import { useEffect, useRef, useState } from "react";
import { Button, Callout, Checkbox, Dialog, Flex, Text, TextField } from "@radix-ui/themes";
import { Choice, Editor, Field } from "../../shared/ui";
import { useWorkbench } from "../workbench/context";
import type { ExportResult, Workspace } from "../../shared/types";
import { api, saveFile } from "../../shared/api";
import { hostConfig, parseHostConfig, type McpHostEntry } from "./config";
export default function McpConfigDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (value: boolean) => void }) {
  useLanguage();
  const state = useWorkbench();
  const [text, setText] = useState("{}");
  const [entries, setEntries] = useState<McpHostEntry[]>([]);
  const [selected, setSelected] = useState("");
  const [name, setName] = useState("MoleAPI");
  const [error, setError] = useState<ErrorCopy>("");
  const [includeSecrets, setIncludeSecrets] = useState(false);
  const [exporting, setExporting] = useState(false);
  const scope = JSON.stringify([state.authenticated,state.accountId,state.draft?.id,state.request?.id,state.draft?.data.active_environment_id,open]);
  const identity = useRef(scope);
  const epoch = useRef(0);
  if (identity.current !== scope) { identity.current = scope; epoch.current++; }
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  useEffect(() => {
    if (!open) return;
    const current = state.request?.protocol;
    setText(current?.kind === "mcp" && current.config_source ? current.config_source : "{}");
    setExporting(false); setIncludeSecrets(false); setName(state.request?.name || "MoleAPI"); setEntries([]); setSelected(""); setError("");
  }, [open]);
  function validate() {
    try { const values = parseHostConfig(text); setEntries(values); setSelected(values[0].name); setError(""); }
    catch (caught) { setError(errorCopy(caught)); setEntries([]); }
  }
  function apply() {
    const generation = epoch.current;
    const entry = entries.find(value => value.name === selected);
    if (!entry || !state.request || state.request.protocol?.kind !== "mcp") return;
    state.setGuard({ title: message("使用 MCP Host 配置"), description: message("当前连接配置与鉴权会被替换，请求参数草稿保留。导入后可检查配置，再手动连接。"), action: () => {
      if (!mounted.current || identity.current !== scope || epoch.current !== generation) return;
      const current = state.request!.protocol;
      if (current?.kind !== "mcp") return;
      state.updateRequest({ url: entry.url, headers: entry.headers, auth: { kind:"none",token:"",username:"",password:"" }, protocol: { ...current, ...entry.config, operation: current.operation, name: current.name, arguments_source: current.arguments_source, uri: current.uri } });
      onOpenChange(false);
    }});
  }
  async function exportCurrent() {
    if (!state.request || exporting) return;
    const origin = scope, generation = epoch.current;
    const current = () => mounted.current && identity.current === origin && epoch.current === generation;
    setExporting(true); setError("");
    try {
      if (!state.draft) return;
      if (state.dirty && !(await state.save(true))) throw new LocalizedError("请先解决保存冲突");
      if (!current()) return;
      const exported = await api<ExportResult>(`/api/workspaces/${encodeURIComponent(state.draft.id)}/export`, "POST", { format: "moleapi", include_secrets: includeSecrets });
      if (!current()) return;
      const workspace: Workspace = JSON.parse(exported.content);
      const request = workspace.data.collections.flatMap(collection => collection.requests).find(request => request.id === state.request!.id);
      if (!request) throw new LocalizedError("请求尚未保存，请保存后重试");
      if (!includeSecrets) request.auth = { kind: "none", token: "", username: "", password: "" };
      await saveFile({ filename:"moleapi-mcp.json",mime:"application/json",content:hostConfig(request,name) });
    }
    catch (caught) { if (current()) setError(errorCopy(caught)); }
    finally { if (current()) setExporting(false); }
  }
  return <Dialog.Root open={open} onOpenChange={onOpenChange}><Dialog.Content maxWidth="900px">
    <Dialog.Title>{t("MCP Host 配置")}</Dialog.Title><Dialog.Description>{t("粘贴 mcpServers 配置或单个服务定义。验证和导入不会连接服务。")}</Dialog.Description>
    <Editor value={text} onChange={value => { setText(value); setEntries([]); }} dark={state.dark} jsonMode height="360px" label="MCP Host JSON" />
    {error && <Callout.Root color="red" role="alert"><Callout.Text>{translateCopy(error)}</Callout.Text></Callout.Root>}
    <Flex gap="3" align="center" mt="3"><Button variant="soft" onClick={validate}>{t("验证配置")}</Button>{entries.length > 0 && <Choice label={t("MCP Host 服务")} value={selected} onChange={setSelected} options={entries.map(entry => ({value:entry.name,label:entry.name}))} />}</Flex>
    <Flex gap="3" align="end" mt="4"><Field label={t("Host 服务名称")}><TextField.Root value={name} onChange={event => setName(event.target.value)} /></Field><Button variant="soft" color="gray" loading={exporting} onClick={() => void exportCurrent()}>{t("导出 Host 配置")}</Button></Flex>
    <label className="checkbox-label"><Checkbox checked={includeSecrets} onCheckedChange={value => setIncludeSecrets(value === true)} disabled={exporting} />{t("包含密钥值")}</label>
    <Text size="1" color="gray">{t("默认排除私密值。完整 Host 配置包含鉴权与环境变量，请妥善保管。")}</Text>
    <Flex gap="3" justify="end" mt="4"><Dialog.Close><Button variant="soft" color="gray">{t("关闭")}</Button></Dialog.Close><Button disabled={!entries.length} onClick={apply}>{t("使用所选配置")}</Button></Flex>
  </Dialog.Content></Dialog.Root>;
}
