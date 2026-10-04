import { useState } from "react";
import { Badge, Button, Callout, Checkbox, Flex, Heading, Table, Text, TextField } from "@radix-ui/themes";
import { native } from "../../shared/api";
import { safeMessage, bytes } from "../../shared/model";
import { Choice } from "../../shared/ui";
import { t, useLanguage, message, translateCopy } from "../../shared/i18n";
import type { LocalizedCopy } from "../../shared/i18n";
import { useWorkbench } from "../workbench/context";
import { useWebhooks } from "./useWebhooks";
import { ReceiverSettings } from "./ReceiverSettings";
import { ReplayPanel } from "./ReplayPanel";
import type { Capture } from "./model";
export default function WebhooksPage() {
  useLanguage();
  const { draft, dark, setGuard } = useWorkbench();
  if (!draft) return null;
  return <WorkspaceWebhooks key={draft.id} workspace={draft.id} dark={dark} guard={setGuard} />;
}
function WorkspaceWebhooks({ workspace, dark, guard }: { workspace: string; dark: boolean; guard: ReturnType<typeof useWorkbench>["setGuard"] }) {
  useLanguage();
  const state = useWebhooks(workspace);
  const [name, setName] = useState("");
  const [captureId, setCaptureId] = useState("");
  const [search, setSearch] = useState("");
  const [host, setHost] = useState("127.0.0.1");
  const [port, setPort] = useState(0);
  const [copyError, setCopyError] = useState<string | LocalizedCopy>("");
  const receiver = state.receiver;
  const captures = state.captures.data?.captures ?? [];
  const capture = captures.find(c => c.id === captureId);
  const origin = native ? state.listener.data?.origin : location.origin;
  const callback = receiver && origin ? `${origin}${receiver.receiver_path}` : "";
  const error = state.error || translateCopy(copyError) || (state.receivers.error && safeMessage(state.receivers.error)) || (state.captures.error && safeMessage(state.captures.error)) || (state.listener.error && safeMessage(state.listener.error));
  return <div className="page-panel webhook-page">
    <div className="page-heading"><div><Heading size="5">{t("Webhook 接收器")}</Heading><Text size="2" color="gray">{t("接收回调、检查原始请求并编辑重放。每个接收器保留最近 64 条记录，最多 8 MiB。")}</Text></div><Button variant="soft" color="gray" disabled={state.busy} onClick={() => void state.act(async () => { await state.receivers.refetch(); if (receiver) await state.captures.refetch(); })}>{t("刷新")}</Button></div>
    {error && <Callout.Root color="red"><Callout.Text>{error}</Callout.Text></Callout.Root>}
    {native && <section className="webhook-listener"><Heading size="3">{t("本地监听")}</Heading><Text size="2" color="gray">{t("桌面端需要显式启动监听。绑定所有接口时，局域网设备也可以发送回调。")}</Text><Flex gap="3" align="end" wrap="wrap"><Choice label={t("监听地址")} value={host} onChange={setHost} options={[{ value: "127.0.0.1", label: t("仅本机") }, { value: "0.0.0.0", label: t("所有接口") }]} /><label><Text as="div" size="2">{t("端口 (0 自动分配)")}</Text><TextField.Root type="number" min={0} max={65535} value={port} onChange={e => setPort(Number(e.target.value))} /></label><Button disabled={state.busy} onClick={() => void (state.listener.data?.active ? state.stop() : state.start(host, port))}>{state.listener.data?.active ? t("停止监听") : t("启动监听")}</Button><Text size="2">{state.listener.data?.bind}</Text></Flex></section>}
    <form onSubmit={e => { e.preventDefault(); void state.create(name); }}><Flex gap="3" align="end" wrap="wrap"><label><Text as="div" size="2">{t("接收器名称")}</Text><TextField.Root required maxLength={256} value={name} onChange={e => setName(e.target.value)} /></label><Button type="submit" disabled={state.busy}>{t("创建接收器")}</Button>{!!state.receivers.data?.length && <Choice label={t("选择接收器")} value={receiver?.id ?? "none"} onChange={id => { state.choose(id); setCaptureId(""); }} options={[{ value: "none", label: t("选择接收器") }, ...state.receivers.data.map(i => ({ value: i.id, label: i.name }))]} />}</Flex></form>
    {state.receivers.isPending && <Text role="status">{t("正在加载接收器…")}</Text>}
    {state.receivers.data?.length === 0 && <Text color="gray">{t("还没有接收器。创建后即可获得专属回调地址。")}</Text>}
    {receiver && <>
      <Flex gap="3" wrap="wrap" align="center"><Badge color={receiver.active ? "green" : "gray"}>{receiver.active ? t("接收中") : t("已暂停")}</Badge><Text size="2">{t("已接收 {{received}} 条 · 已淘汰 {{dropped}} 条", { received: receiver.received, dropped: receiver.dropped })}</Text><Button variant="soft" disabled={state.busy} onClick={() => void state.update({ ...receiver, active: !receiver.active })}>{receiver.active ? t("暂停接收") : t("恢复接收")}</Button><Button color="red" variant="soft" disabled={state.busy} onClick={() => guard({ title: message("删除接收器"), description: message("删除接收器及其记录，原回调地址将失效。"), action: () => state.remove(receiver) })}>{t("删除接收器")}</Button></Flex>
      <label><Text as="div" size="2">{t("回调地址（持有地址即可发送）")}</Text><Flex gap="2"><TextField.Root className="webhook-destination mono" readOnly value={callback} aria-label={t("回调地址")} /><Button disabled={!callback} variant="soft" onClick={async () => { setCopyError(""); if (!navigator.clipboard) { setCopyError(message("剪贴板不可用")); return; } try { await navigator.clipboard.writeText(callback); } catch (e) { setCopyError(safeMessage(e)); } }}>{t("复制")}</Button></Flex></label>
      {!callback && <Text size="2" color="gray">{t("启动本地监听后显示回调地址。")}</Text>}
      <ReceiverSettings key={`${receiver.id}/${receiver.config_epoch}`} receiver={receiver} busy={state.busy} save={state.update} />
      <Flex gap="3" wrap="wrap" align="center"><Heading size="3">{t("捕获记录")}</Heading><TextField.Root aria-label={t("筛选记录")} placeholder={t("筛选记录")} value={search} onChange={e => setSearch(e.target.value)} /><Text as="label" size="2"><Checkbox checked={state.privateView} onCheckedChange={v => { setCaptureId(""); state.setPrivateView(v === true); }} /> {t("显示及导出私密原文")}</Text><Button disabled={state.busy} variant="soft" onClick={() => void state.export(receiver)}>{t("导出记录")}</Button><Button disabled={state.busy} variant="soft" color="gray" onClick={() => guard({ title: message("清空记录"), description: message("清空捕获记录，回调地址和回复配置仍然保留。"), action: () => state.clear(receiver) })}>{t("清空记录")}</Button></Flex>
      {state.privateView && <Callout.Root color="orange"><Callout.Text>{t("私密原文可能包含凭据、个人数据和二进制内容。导出将包含回调密钥。")}</Callout.Text></Callout.Root>}
      <Table.Root><Table.Header><Table.Row><Table.ColumnHeaderCell>{t("序号")}</Table.ColumnHeaderCell><Table.ColumnHeaderCell>{t("方法")}</Table.ColumnHeaderCell><Table.ColumnHeaderCell>{t("时间")}</Table.ColumnHeaderCell><Table.ColumnHeaderCell>{t("大小")}</Table.ColumnHeaderCell><Table.ColumnHeaderCell>{t("操作")}</Table.ColumnHeaderCell></Table.Row></Table.Header><Table.Body>{captures.filter(c => !search || JSON.stringify(c).toLowerCase().includes(search.toLowerCase())).map(c => <Table.Row key={c.id}><Table.Cell>#{c.cursor}</Table.Cell><Table.Cell><span className={`method method-${c.method.toLowerCase()}`}>{c.method}</span></Table.Cell><Table.Cell>{new Date(c.received_at).toLocaleString()}</Table.Cell><Table.Cell>{bytes(c.body_bytes)}</Table.Cell><Table.Cell><Button variant="ghost" onClick={() => setCaptureId(c.id)}>{t("检查请求")}</Button></Table.Cell></Table.Row>)}</Table.Body></Table.Root>
      {captures.length === 0 && <Text color="gray">{t("等待回调请求…")}</Text>}
      {capture && <CaptureDetails key={`${capture.id}/${state.privateView}`} capture={capture} inbox={receiver.id} dark={dark} />}
    </>}
  </div>;
}
function CaptureDetails({ capture, inbox, dark }: { capture: Capture; inbox: string; dark: boolean }) {
  useLanguage();
  return <section className="webhook-details"><Heading size="3">{t("请求详情")} #{capture.cursor}</Heading>{capture.redacted && <Text size="2" color="gray">{t("已隐藏敏感值。二进制正文需开启私密原文查看。")}</Text>}<Text as="div" size="2">{t("查询参数")}</Text><pre tabIndex={0} className="webhook-content">{capture.query || "—"}</pre><Text as="div" size="2">{t("请求头")}</Text><pre tabIndex={0} className="webhook-content">{capture.headers.map(h => `${h.name}: ${h.value_text ?? `[Base64] ${h.value_base64}`}`).join("\n")}</pre><Text as="div" size="2">{t("正文")}</Text><pre tabIndex={0} className="webhook-content">{capture.body_text ?? capture.body_base64}</pre><ReplayPanel inbox={inbox} capture={capture} dark={dark} /></section>;
}
