import { useEffect, useId, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertDialog, Button, Callout, Flex, Text, TextField } from "@radix-ui/themes";
import { api, native, token } from "../../shared/api";
import { t, useLanguage } from "../../shared/i18n";
import { liveError } from "../../shared/i18n/errors";
import { Choice, Field } from "../../shared/ui";
import { useWorkbench } from "../workbench/context";
import NotificationPicker from "../notifications/NotificationPicker";
import type { ExecutionRunner, RunnerSelection, RunnerTask } from "./types";

export default function RunnersPanel({ onReport }: { onReport: (id: string) => void }) {
  const state = useWorkbench(), { language } = useLanguage(), client = useQueryClient();
  const instance = useId(), credential = token(), workspace = state.draft;
  const boundary = useRef({ credential, epoch: 0 });
  if (boundary.current.credential !== credential) boundary.current = { credential, epoch: boundary.current.epoch + 1 };
  const scope = JSON.stringify([state.accountId, workspace?.id, boundary.current.epoch, state.authenticated]);
  const latest = useRef(scope); latest.current = scope;
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const current = (origin: string) => mounted.current && latest.current === origin && token() === credential;
  const [name, setName] = useState("");
  const [editing, setEditing] = useState<ExecutionRunner | null>(null);
  const [disabling, setDisabling] = useState<ExecutionRunner | null>(null);
  const [selection, setSelection] = useState<RunnerSelection>({ collection_id: workspace?.data.collections[0]?.id ?? "", scenario_id: null, environment_id: null, dataset_id: null, iterations: null, notification_ids: [] });
  const [runnerId, setRunnerId] = useState("");
  const [pending, setPending] = useState<string | null>(null);
  const [error, setError] = useState<ReturnType<typeof liveError>>("");
  useEffect(() => { setName(""); setEditing(null); setDisabling(null); setRunnerId(""); setError(""); setPending(null); setSelection({ collection_id: workspace?.data.collections[0]?.id ?? "", scenario_id: null, environment_id: null, dataset_id: null, iterations: null, notification_ids: [] }); }, [scope]);
  const queryKey = ["execution-runners", state.accountId, instance, boundary.current.epoch];
  const taskKey = ["runner-tasks", state.accountId, workspace?.id, instance, boundary.current.epoch];
  const enabled = !native && state.authenticated && !!credential;
  const runners = useQuery({ queryKey, queryFn: () => current(scope) ? api<ExecutionRunner[]>("/api/runners") : [], enabled, refetchInterval: 5000, retry: false });
  const tasks = useQuery({ queryKey: taskKey, queryFn: () => current(scope) ? api<RunnerTask[]>(`/api/workspaces/${encodeURIComponent(workspace!.id)}/runner-tasks`) : [], enabled: enabled && !!workspace, refetchInterval: 3000, retry: false });
  const busy = pending === scope;
  async function action(operation: () => Promise<unknown>) {
    if (busy || !current(scope)) return;
    const origin = scope; setPending(origin); setError("");
    try {
      await operation();
      if (current(origin)) {
        setEditing(null); setDisabling(null); setName("");
        void client.invalidateQueries({ queryKey }); void client.invalidateQueries({ queryKey: taskKey });
        void client.invalidateQueries({ queryKey: ["schedules", state.accountId, workspace?.id] });
      }
    } catch (error) { if (current(origin)) setError(liveError(error)); }
    finally { if (current(origin)) setPending(null); }
  }
  function update(runner: ExecutionRunner, enabled: boolean) {
    return api(`/api/runners/${encodeURIComponent(runner.id)}`, "PATCH", { name: runner.name.trim(), enabled, expected_revision: runner.revision });
  }
  async function queue() {
    const origin = scope;
    await action(async () => {
      const saved = state.dirty ? await state.save(true) : workspace;
      if (!saved || saved.id !== workspace?.id || !current(origin)) return;
      await api(`/api/workspaces/${encodeURIComponent(saved.id)}/runner-tasks`, "POST", { runner_id: runnerId, expected_revision: saved.revision, selection, max_attempts: 1 });
    });
  }
  const change = (patch: Partial<RunnerSelection>) => setSelection(value => ({ ...value, ...patch }));
  const date = (value: string) => new Date(value).toLocaleString(language);
  const status = (value: RunnerTask["status"]) => value === "queued" ? t("已排队") : value === "leased" ? t("运行中") : value === "completed" ? t("结果已接收") : value === "failed" ? t("失败") : t("已取消");
  return <details><summary>{t("自托管执行器")}</summary><Flex direction="column" gap="3" mt="3">
    {native ? <Text size="2" color="gray">{t("执行器管理需要打开托管服务网页；离线桌面仍可直接运行集合。")}</Text> : <>
      <Text size="2" color="gray">{t("执行器在自己的网络中运行任务，会收到所选工作区的请求凭据。仅注册可信机器。")}</Text>
      <Text size="1" color="gray">{t("注册后使用 CLI runner agent 启动对应 ID。每个执行器同时运行一个任务；最近联系时间不保证当前在线。")}</Text>
      {(error || runners.error || tasks.error) && <Callout.Root color="red" role="alert"><Callout.Text>{error || liveError(runners.error || tasks.error)}</Callout.Text></Callout.Root>}
      <Flex gap="3" wrap="wrap" align="end"><Field label={t("执行器名称")}><TextField.Root value={name} maxLength={80} disabled={busy} onChange={event => setName(event.target.value)} /></Field><Button disabled={busy || !name.trim()} loading={busy} onClick={() => void action(() => api("/api/runners", "POST", { name: name.trim() }))}>{t("注册执行器")}</Button><Button variant="soft" disabled={busy} onClick={() => { void runners.refetch(); void tasks.refetch(); }}>{t("刷新执行器")}</Button></Flex>
      {runners.data?.length === 0 && <Text color="gray" size="2">{t("尚无自托管执行器")}</Text>}
      {runners.data?.map(runner => <Flex key={runner.id} direction="column" gap="2" py="2" style={{ borderBottom: "1px solid var(--gray-5)", overflowWrap: "anywhere" }}>
        <Flex gap="3" wrap="wrap" align="center"><Text weight="medium">{runner.name}</Text><Text size="1" color="gray">{runner.enabled ? t("已启用") : t("已停用")} · {runner.active_task_id ? t("有活动任务") : t("无活动任务")}</Text></Flex>
        <Text size="1" color="gray">ID: {runner.id}</Text><Text size="1" color="gray">{t("最近联系")}: {runner.last_seen_at ? date(runner.last_seen_at) : t("尚未连接")}</Text>
        <Flex gap="2" wrap="wrap"><Button variant="soft" disabled={busy} onClick={() => setEditing({ ...runner })}>{t("重命名执行器")}</Button><Button variant="soft" disabled={busy} onClick={() => runner.enabled ? setDisabling(runner) : void action(() => update(runner, true))}>{runner.enabled ? t("停用执行器") : t("启用执行器")}</Button></Flex>
        {editing?.id === runner.id && <Flex gap="2" wrap="wrap" align="end"><Field label={t("新的执行器名称")}><TextField.Root value={editing.name} maxLength={80} disabled={busy} onChange={event => setEditing({ ...editing, name: event.target.value })} /></Field><Button disabled={busy || !editing.name.trim()} onClick={() => void action(() => update(editing, editing.enabled))}>{t("保存执行器名称")}</Button><Button variant="soft" disabled={busy} onClick={() => setEditing(null)}>{t("取消")}</Button></Flex>}
      </Flex>)}
      {workspace && <><Text weight="medium">{t("派发保存的测试")}</Text><Flex gap="3" wrap="wrap"><Choice label={t("目标执行器")} value={runnerId || "none"} disabled={busy} options={[{ value: "none", label: t("请选择") }, ...(runners.data ?? []).filter(runner => runner.enabled).map(runner => ({ value: runner.id, label: runner.name }))]} onChange={value => setRunnerId(value === "none" ? "" : value)} /><Choice label={t("任务集合")} value={selection.collection_id || "none"} disabled={busy} options={workspace.data.collections.map(collection => ({ value: collection.id, label: collection.name }))} onChange={collection_id => change({ collection_id, scenario_id: null })} /></Flex>
        <Flex gap="3" wrap="wrap"><Choice label={t("任务场景")} value={selection.scenario_id ?? "none"} disabled={busy} options={[{ value: "none", label: t("集合原始顺序") }, ...(workspace.data.scenarios ?? []).filter(scenario => scenario.collection_id === selection.collection_id).map(scenario => ({ value: scenario.id, label: scenario.name }))]} onChange={value => change({ scenario_id: value === "none" ? null : value })} /><Choice label={t("任务环境")} value={selection.environment_id ?? "none"} disabled={busy} options={[{ value: "none", label: t("使用工作区当前环境") }, ...workspace.data.environments.map(environment => ({ value: environment.id, label: environment.name }))]} onChange={value => change({ environment_id: value === "none" ? null : value })} /><Choice label={t("任务数据集")} value={selection.dataset_id ?? "none"} disabled={busy} options={[{ value: "none", label: t("不选择数据集") }, ...(workspace.data.datasets ?? []).filter(dataset => dataset.source).map(dataset => ({ value: dataset.id, label: dataset.name }))]} onChange={value => change({ dataset_id: value === "none" ? null : value })} /><Field label={t("任务迭代次数（空值使用数据行数）")}><TextField.Root type="number" min={1} max={100} disabled={busy} value={selection.iterations ?? ""} onChange={event => change({ iterations: event.target.value ? Number(event.target.value) : null })} /></Field></Flex>
        <NotificationPicker label={t("指定通知对象")} value={selection.notification_ids} disabled={busy} onChange={notification_ids => change({ notification_ids })} />
        <Text size="1" color="gray">{t("派发前保存当前工作区，使用保存的环境与数据集，不包含浏览器本地覆盖。任务只尝试一次，不自动重放。")}</Text>
        <Button disabled={busy || !selection.collection_id || !(runners.data ?? []).some(runner => runner.id === runnerId && runner.enabled)} loading={busy} onClick={() => void queue()}>{t("派发到执行器")}</Button>
        <Text weight="medium">{t("执行器任务记录")}</Text>{tasks.data?.length === 0 && <Text size="2" color="gray">{t("暂无执行器任务")}</Text>}
        {tasks.data?.map(task => <Flex key={task.id} direction="column" gap="2" py="2" style={{ borderBottom: "1px solid var(--gray-5)", overflowWrap: "anywhere" }}><Flex gap="3" wrap="wrap"><Text>{(runners.data ?? []).find(runner => runner.id === task.runner_id)?.name ?? task.runner_id}</Text><Text>{status(task.status)}</Text><Text size="1">{t("来源工作区版本")} {task.source_revision} · {task.attempt}/{task.max_attempts}</Text></Flex><Text size="1" color="gray">{date(task.queued_at)} · {task.id}</Text><Flex gap="2" wrap="wrap">{(task.status === "queued" || task.status === "leased") && <Button variant="soft" disabled={busy} onClick={() => void action(() => api(`/api/workspaces/${encodeURIComponent(workspace.id)}/runner-tasks/${encodeURIComponent(task.id)}/cancel`, "POST"))}>{t("取消任务执行")}</Button>}{task.report_id && <Button variant="soft" onClick={() => onReport(task.report_id!)}>{t("查看任务报告")}</Button>}</Flex></Flex>)}
        <Text size="1" color="gray">{t("结果已接收表示报告已保存，断言是否通过请查看报告。取消将在执行器下次续租时停止本地执行。")}</Text>
      </>}
      <AlertDialog.Root open={!!disabling} onOpenChange={open => { if (!open && !busy) setDisabling(null); }}><AlertDialog.Content><AlertDialog.Title>{t("停用执行器")}</AlertDialog.Title><AlertDialog.Description>{t("停用后不再领取任务，活动任务会被取消。确认执行器名称后继续。")}</AlertDialog.Description><Text as="p" mt="2">{disabling?.name}</Text><Flex justify="end" gap="3" mt="4"><AlertDialog.Cancel><Button variant="soft" disabled={busy}>{t("取消")}</Button></AlertDialog.Cancel><Button color="red" disabled={busy} loading={busy} onClick={() => disabling && void action(() => update(disabling, false))}>{t("确认停用")}</Button></Flex></AlertDialog.Content></AlertDialog.Root>
    </>}
  </Flex></details>;
}
