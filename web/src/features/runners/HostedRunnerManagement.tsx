import { useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { AlertDialog, Button, Callout, Flex, Text, TextArea, TextField } from "@radix-ui/themes";
import { api } from "../../shared/api";
import { t, useLanguage } from "../../shared/i18n";
import { liveError } from "../../shared/i18n/errors";
import { Choice, Field } from "../../shared/ui";
import type { HistoryEntry, SavedRunReport, ScheduleDefinition, ScheduledRun, Workspace } from "../../shared/types";
import RunnerCredentials from "./RunnerCredentials";
import type { ExecutionRunner, RunnerSelection, RunnerTask } from "./types";

const hosted = <T,>(path: string, method = "GET", body?: unknown) =>
  api<T>(path.replace(/^\/api\//, "/api/hosted/"), method, body);
const labels = (value: string) => value.split(",").map(part => part.trim().toLowerCase()).filter(Boolean);
const targetFields = (value: string) => value.startsWith("label:")
  ? { runner_id: null, runner_label: value.slice(6) }
  : { runner_id: value || null, runner_label: null };
const newSchedule = (collection: string): ScheduleDefinition => ({
  runner_id: null, runner_label: null, notification_ids: [], name: "", cron: "0 9 * * *",
  timezone: "UTC", enabled: false, collection_id: collection, scenario_id: null,
  environment_id: null, dataset_id: null, iterations: null,
});

export default function HostedRunnerManagement({ connectionId, workspaceId }: { connectionId: string; workspaceId: string }) {
  useLanguage();
  const scope = `${connectionId}:${workspaceId}`;
  const currentScope = useRef(scope); currentScope.current = scope;
  const mounted = useRef(true);
  const reportEpoch = useRef(0), responseEpoch = useRef(0);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const current = (origin: string) => mounted.current && currentScope.current === origin;
  const path = `/api/workspaces/${encodeURIComponent(workspaceId)}`;
  const source = useQuery({ queryKey: ["hosted-workspace", scope], queryFn: () => hosted<Workspace>(path), retry: false });
  const runners = useQuery({ queryKey: ["hosted-runners", scope], queryFn: () => hosted<ExecutionRunner[]>("/api/runners"), retry: false });
  const tasks = useQuery({ queryKey: ["hosted-runner-tasks", scope], queryFn: () => hosted<RunnerTask[]>(`${path}/runner-tasks`), enabled: !!source.data, retry: false });
  const schedules = useQuery({ queryKey: ["hosted-schedules", scope], queryFn: () => hosted<ScheduledRun[]>(`${path}/schedules`), enabled: !!source.data, retry: false });
  const [name, setName] = useState(""), [newLabels, setNewLabels] = useState("");
  const [target, setTarget] = useState(""), [collectionId, setCollectionId] = useState("");
  const [scenarioId, setScenarioId] = useState(""), [environmentId, setEnvironmentId] = useState("");
  const [datasetId, setDatasetId] = useState(""), [iterations, setIterations] = useState("");
  const [credentialRunner, setCredentialRunner] = useState<string | null>(null);
  const [editRunner, setEditRunner] = useState<{ runner: ExecutionRunner; name: string; labels: string } | null>(null);
  const [disableRunner, setDisableRunner] = useState<ExecutionRunner | null>(null);
  const [schedule, setSchedule] = useState<{ id: string; revision: number; definition: ScheduleDefinition }>(() => ({ id: "", revision: 0, definition: newSchedule("") }));
  const [deleteSchedule, setDeleteSchedule] = useState<ScheduledRun | null>(null);
  const [report, setReport] = useState<SavedRunReport | null>(null), [response, setResponse] = useState<HistoryEntry | null>(null);
  const [busy, setBusy] = useState(false), [error, setError] = useState<ReturnType<typeof liveError>>("");
  const workspace = source.data, data = workspace?.data;
  const selectedCollection = collectionId || data?.collections[0]?.id || "";
  const groupLabels = [...new Set((runners.data ?? []).filter(runner => runner.enabled).flatMap(runner => runner.labels ?? []))].sort();
  const targetOptions = [
    { value: "none", label: t("请选择") },
    ...(runners.data ?? []).filter(runner => runner.enabled).map(runner => ({ value: runner.id, label: runner.name })),
    ...groupLabels.map(label => ({ value: `label:${label}`, label: `${t("标签组")} #${label}` })),
  ];
  const validTarget = target.startsWith("label:")
    ? groupLabels.includes(target.slice(6))
    : (runners.data ?? []).some(runner => runner.id === target && runner.enabled);
  const definition = schedule.definition;
  const scheduleTarget = definition.runner_label ? `label:${definition.runner_label}` : definition.runner_id ?? "none";
  const scheduleOptions = [
    { value: "none", label: t("当前服务端") },
    ...targetOptions.slice(1),
    ...(scheduleTarget !== "none" && !targetOptions.some(option => option.value === scheduleTarget)
      ? [{ value: scheduleTarget, label: t("执行器不可用") }] : []),
  ];
  const changeSchedule = (patch: Partial<ScheduleDefinition>) => setSchedule(value => ({ ...value, definition: { ...value.definition, ...patch } }));
  async function action(operation: () => Promise<unknown>, after?: () => void) {
    if (busy) return;
    const origin = scope; setBusy(true); setError("");
    try {
      await operation();
      if (current(origin)) {
        after?.();
        await Promise.all([source.refetch(), runners.refetch(), tasks.refetch(), schedules.refetch()]);
      }
    } catch (cause) { if (current(origin)) setError(liveError(cause)); }
    finally { if (current(origin)) setBusy(false); }
  }
  async function openReport(id: string) {
    const origin = scope, epoch = ++reportEpoch.current; ++responseEpoch.current;
    setReport(null); setResponse(null); setError("");
    try {
      const value = await hosted<SavedRunReport>(`${path}/reports/${encodeURIComponent(id)}`);
      if (current(origin) && reportEpoch.current === epoch) setReport(value);
    } catch (cause) { if (current(origin) && reportEpoch.current === epoch) setError(liveError(cause)); }
  }
  async function openResponse(position: number) {
    if (!report) return;
    const origin = scope, epoch = ++responseEpoch.current; setResponse(null); setError("");
    try {
      const value = await hosted<HistoryEntry>(`${path}/reports/${encodeURIComponent(report.id)}/steps/${position}/response`);
      if (current(origin) && responseEpoch.current === epoch) setResponse(value);
    } catch (cause) { if (current(origin) && responseEpoch.current === epoch) setError(liveError(cause)); }
  }
  function selection(): RunnerSelection {
    return { collection_id: selectedCollection, scenario_id: scenarioId || null, environment_id: environmentId || null,
      dataset_id: datasetId || null, iterations: iterations ? Number(iterations) : null, notification_ids: [] };
  }
  return <details><summary>{t("托管执行器管理")}</summary><Flex direction="column" gap="3" mt="3">
    <Text size="2">{t("此区域操作已连接服务端的工作区；离线工作区不会随执行器操作而修改。")}</Text>
    {source.isPending && <Text size="2">{t("正在加载托管工作区")}</Text>}
    {source.error && <Callout.Root color="amber" role="alert"><Callout.Text>{t("托管端没有此工作区；请先同步当前工作区。")}</Callout.Text></Callout.Root>}
    {workspace && <Text size="2">{t("托管工作区版本")} {workspace.revision} · {workspace.name}</Text>}
    {(error || runners.error || tasks.error || schedules.error) && <Callout.Root color="red" role="alert"><Callout.Text>{error || liveError(runners.error || tasks.error || schedules.error)}</Callout.Text></Callout.Root>}
    <Flex gap="2" wrap="wrap" align="end"><Field label={t("执行器名称")}><TextField.Root value={name} maxLength={80} disabled={busy} onChange={event => setName(event.target.value)} /></Field><Field label={t("执行器标签（逗号分隔）")}><TextField.Root value={newLabels} disabled={busy} onChange={event => setNewLabels(event.target.value)} /></Field><Button disabled={busy || !name.trim()} onClick={() => void action(() => hosted("/api/runners", "POST", { name: name.trim(), labels: labels(newLabels) }), () => { setName(""); setNewLabels(""); })}>{t("注册执行器")}</Button></Flex>
    {runners.data?.map(runner => <Flex key={runner.id} direction="column" gap="2" py="2" style={{ borderBottom: "1px solid var(--gray-5)" }}>
      <Text weight="medium">{runner.name} · {runner.enabled ? t("已启用") : t("已停用")}</Text><Text size="1">{runner.id} · {(runner.labels ?? []).map(label => `#${label}`).join(" ")}</Text>
      <Flex gap="2" wrap="wrap"><Button variant="soft" disabled={busy} onClick={() => setEditRunner({ runner, name: runner.name, labels: (runner.labels ?? []).join(", ") })}>{t("编辑执行器")}</Button><Button variant="soft" disabled={busy} onClick={() => runner.enabled ? setDisableRunner(runner) : void action(() => hosted(`/api/runners/${encodeURIComponent(runner.id)}`, "PATCH", { name: runner.name, enabled: true, expected_revision: runner.revision }))}>{runner.enabled ? t("停用执行器") : t("启用执行器")}</Button>{workspace && <Button variant="soft" onClick={() => setCredentialRunner(value => value === runner.id ? null : runner.id)}>{t("管理执行器凭据")}</Button>}</Flex>
      {editRunner?.runner.id === runner.id && <Flex gap="2" wrap="wrap" align="end"><Field label={t("新的执行器名称")}><TextField.Root value={editRunner.name} disabled={busy} onChange={event => setEditRunner({ ...editRunner, name: event.target.value })} /></Field><Field label={t("执行器标签（逗号分隔）")}><TextField.Root value={editRunner.labels} disabled={busy} onChange={event => setEditRunner({ ...editRunner, labels: event.target.value })} /></Field><Button disabled={busy || !editRunner.name.trim()} onClick={() => void action(() => hosted(`/api/runners/${encodeURIComponent(runner.id)}`, "PATCH", { name: editRunner.name.trim(), labels: labels(editRunner.labels), enabled: runner.enabled, expected_revision: runner.revision }), () => setEditRunner(null))}>{t("保存执行器")}</Button><Button variant="ghost" onClick={() => setEditRunner(null)}>{t("取消")}</Button></Flex>}
      {workspace && credentialRunner === runner.id && <RunnerCredentials accountId={scope} runnerId={runner.id} workspaceId={workspaceId} connectionKey={connectionId} request={hosted} onClose={() => setCredentialRunner(null)} />}
    </Flex>)}
    {workspace && <><Text weight="medium">{t("派发托管测试")}</Text><Flex gap="2" wrap="wrap"><Choice label={t("目标执行器")} value={target || "none"} options={targetOptions} disabled={busy} onChange={value => setTarget(value === "none" ? "" : value)} /><Choice label={t("任务集合")} value={selectedCollection || "none"} options={data!.collections.map(collection => ({ value: collection.id, label: collection.name }))} disabled={busy} onChange={value => { setCollectionId(value); setScenarioId(""); }} /><Choice label={t("任务场景")} value={scenarioId || "none"} options={[{ value: "none", label: t("集合原始顺序") }, ...(data?.scenarios ?? []).filter(scenario => scenario.collection_id === selectedCollection).map(scenario => ({ value: scenario.id, label: scenario.name }))]} disabled={busy} onChange={value => setScenarioId(value === "none" ? "" : value)} /><Choice label={t("任务环境")} value={environmentId || "none"} options={[{ value: "none", label: t("使用工作区当前环境") }, ...data!.environments.map(environment => ({ value: environment.id, label: environment.name }))]} disabled={busy} onChange={value => setEnvironmentId(value === "none" ? "" : value)} /><Choice label={t("任务数据集")} value={datasetId || "none"} options={[{ value: "none", label: t("不选择数据集") }, ...(data?.datasets ?? []).filter(dataset => dataset.source).map(dataset => ({ value: dataset.id, label: dataset.name }))]} disabled={busy} onChange={value => setDatasetId(value === "none" ? "" : value)} /><Field label={t("任务迭代次数（空值使用数据行数）")}><TextField.Root type="number" min={1} max={100} value={iterations} disabled={busy} onChange={event => setIterations(event.target.value)} /></Field></Flex>
      <Button disabled={busy || !validTarget || !selectedCollection} onClick={() => void action(() => hosted(`${path}/runner-tasks`, "POST", { ...targetFields(target), expected_revision: workspace.revision, selection: selection(), max_attempts: 1 }))}>{t("派发到执行器")}</Button>
      <Text weight="medium">{t("执行器任务记录")}</Text>{tasks.data?.map(task => <Flex key={task.id} gap="2" wrap="wrap" align="center"><Text size="2">{(runners.data ?? []).find(runner => runner.id === task.runner_id)?.name ?? task.runner_id} · {task.status}{task.target_label ? ` · #${task.target_label}` : ""}</Text>{(task.status === "queued" || task.status === "leased") && <Button variant="soft" disabled={busy} onClick={() => void action(() => hosted(`${path}/runner-tasks/${encodeURIComponent(task.id)}/cancel`, "POST"))}>{t("取消任务执行")}</Button>}{task.report_id && <Button variant="soft" onClick={() => void openReport(task.report_id!)}>{t("查看任务报告")}</Button>}</Flex>)}</>}
    {workspace && <><Text weight="medium">{t("托管计划任务")}</Text>{schedules.data?.map(item => <Flex key={item.id} gap="2" wrap="wrap" align="center"><Text size="2">{item.definition.name} · {item.definition.cron} · {item.last_run?.status ?? ""}</Text><Button variant="soft" onClick={() => setSchedule({ id: item.id, revision: item.revision, definition: structuredClone(item.definition) })}>{t("编辑任务")}</Button><Button variant="soft" disabled={busy} onClick={() => void action(() => hosted(`${path}/schedules/${encodeURIComponent(item.id)}/run`, "POST"))}>{t("立即排队运行")}</Button><Button variant="soft" disabled={busy} onClick={() => void action(() => hosted(`${path}/schedules/${encodeURIComponent(item.id)}/cancel`, "POST"))}>{t("取消任务执行")}</Button><Button variant="ghost" color="red" onClick={() => setDeleteSchedule(item)}>{t("删除任务")}</Button>{item.last_run?.report_id && <Button variant="soft" onClick={() => void openReport(item.last_run!.report_id!)}>{t("查看任务报告")}</Button>}</Flex>)}
      <Button variant="soft" onClick={() => setSchedule({ id: "", revision: 0, definition: newSchedule(data?.collections[0]?.id ?? "") })}>{t("新建定时任务")}</Button>
      <Flex gap="2" wrap="wrap" align="end"><Field label={t("任务名称")}><TextField.Root value={definition.name} disabled={busy} onChange={event => changeSchedule({ name: event.target.value })} /></Field><Field label={t("Cron 表达式（秒可选）")}><TextField.Root value={definition.cron} disabled={busy} onChange={event => changeSchedule({ cron: event.target.value })} /></Field><Field label={t("IANA 时区")}><TextField.Root value={definition.timezone} disabled={busy} onChange={event => changeSchedule({ timezone: event.target.value })} /></Field><Choice label={t("计划执行位置")} value={scheduleTarget} options={scheduleOptions} disabled={busy} onChange={value => changeSchedule(targetFields(value === "none" ? "" : value))} /><Choice label={t("任务集合")} value={definition.collection_id || data?.collections[0]?.id || "none"} options={data!.collections.map(collection => ({ value: collection.id, label: collection.name }))} disabled={busy} onChange={value => changeSchedule({ collection_id: value, scenario_id: null })} /><Choice label={t("任务场景")} value={definition.scenario_id ?? "none"} options={[{ value: "none", label: t("集合原始顺序") }, ...(data?.scenarios ?? []).filter(item => item.collection_id === (definition.collection_id || data?.collections[0]?.id)).map(item => ({ value: item.id, label: item.name }))]} disabled={busy} onChange={value => changeSchedule({ scenario_id: value === "none" ? null : value })} /><Choice label={t("任务环境")} value={definition.environment_id ?? "none"} options={[{ value: "none", label: t("使用工作区当前环境") }, ...data!.environments.map(item => ({ value: item.id, label: item.name }))]} disabled={busy} onChange={value => changeSchedule({ environment_id: value === "none" ? null : value })} /><Choice label={t("任务数据集")} value={definition.dataset_id ?? "none"} options={[{ value: "none", label: t("不选择数据集") }, ...(data?.datasets ?? []).filter(item => item.source).map(item => ({ value: item.id, label: item.name }))]} disabled={busy} onChange={value => changeSchedule({ dataset_id: value === "none" ? null : value })} /></Flex>
      <label><input type="checkbox" checked={definition.enabled} disabled={busy} onChange={event => changeSchedule({ enabled: event.target.checked })} /> {t("启用计划运行")}</label>
      <Button disabled={busy || !definition.name.trim() || !(definition.collection_id || data?.collections[0]?.id)} onClick={() => void action(() => {
        const body = { ...definition, collection_id: definition.collection_id || data!.collections[0].id };
        return schedule.id ? hosted(`${path}/schedules/${encodeURIComponent(schedule.id)}`, "PUT", { definition: body, expected_revision: schedule.revision }) : hosted(`${path}/schedules`, "POST", body);
      }, () => setSchedule({ id: "", revision: 0, definition: newSchedule(data?.collections[0]?.id ?? "") }))}>{t("保存定时任务")}</Button></>}
    {report && <section aria-label={t("托管任务报告")}><Flex direction="column" gap="2"><Flex justify="between"><Text weight="medium">{t("托管任务报告")} · {report.summary.passed} / {report.summary.failed}</Text><Button variant="ghost" onClick={() => { ++reportEpoch.current; ++responseEpoch.current; setReport(null); setResponse(null); }}>{t("关闭")}</Button></Flex>{report.results.map(step => <Flex key={step.position} gap="2" wrap="wrap" align="center"><Text size="2">{step.position + 1}. {step.request_name} · {step.status ?? step.outcome}</Text>{step.history_id && <Button variant="soft" onClick={() => void openResponse(step.position)}>{t("查看响应")}</Button>}</Flex>)}{response && <TextArea aria-label={t("托管响应预览")} readOnly value={`${response.response.status} ${response.response.status_text}\n${response.response.body}`} />}</Flex></section>}
    <AlertDialog.Root open={!!disableRunner} onOpenChange={open => { if (!open && !busy) setDisableRunner(null); }}><AlertDialog.Content><AlertDialog.Title>{t("停用执行器")}</AlertDialog.Title><AlertDialog.Description>{t("停用后不再领取任务，已排队和活动任务会被取消。确认执行器名称后继续。")}</AlertDialog.Description><Text as="p">{disableRunner?.name}</Text><Flex justify="end" gap="2" mt="3"><AlertDialog.Cancel><Button variant="soft">{t("取消")}</Button></AlertDialog.Cancel><Button color="red" disabled={busy} onClick={() => disableRunner && void action(() => hosted(`/api/runners/${encodeURIComponent(disableRunner.id)}`, "PATCH", { name: disableRunner.name, enabled: false, expected_revision: disableRunner.revision }), () => setDisableRunner(null))}>{t("确认停用")}</Button></Flex></AlertDialog.Content></AlertDialog.Root>
    <AlertDialog.Root open={!!deleteSchedule} onOpenChange={open => { if (!open && !busy) setDeleteSchedule(null); }}><AlertDialog.Content><AlertDialog.Title>{t("删除任务")}</AlertDialog.Title><AlertDialog.Description>{t("删除托管计划任务及其待执行任务。")}</AlertDialog.Description><Text as="p">{deleteSchedule?.definition.name}</Text><Flex justify="end" gap="2" mt="3"><AlertDialog.Cancel><Button variant="soft">{t("取消")}</Button></AlertDialog.Cancel><Button color="red" disabled={busy} onClick={() => deleteSchedule && void action(() => hosted(`${path}/schedules/${encodeURIComponent(deleteSchedule.id)}`, "DELETE", { expected_revision: deleteSchedule.revision }), () => setDeleteSchedule(null))}>{t("确认删除")}</Button></Flex></AlertDialog.Content></AlertDialog.Root>
  </Flex></details>;
}
