import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Button, Callout, Flex, Heading, Text, TextArea, TextField } from "@radix-ui/themes";
import { api, token } from "../../shared/api";
import { t, useLanguage } from "../../shared/i18n";
import { liveError } from "../../shared/i18n/errors";
import { id } from "../../shared/model";
import type { HistoryEntry, RequestSpec, RunReportPage, SharedWorkspaceSummary, Workspace } from "../../shared/types";
import { Choice, Field } from "../../shared/ui";
import { hasChanges } from "./draft";

type Tab = "resources" | "history" | "reports";
type RunResult = { passed: number; failed: number; cancelled: boolean; report_id?: string };

export default function SharedWorkspaceView({ accountId, selected, onClose, onDirtyChange }: { accountId: string; selected: SharedWorkspaceSummary; onClose: () => void; onDirtyChange: (dirty: boolean) => void }) {
  useLanguage();
  const client = useQueryClient();
  const credential = token();
  const boundary = useRef({ accountId, credential, epoch: 0 });
  if (boundary.current.accountId !== accountId || boundary.current.credential !== credential) boundary.current = { accountId, credential, epoch: boundary.current.epoch + 1 };
  const epoch = boundary.current.epoch;
  const owner = selected.owner_id;
  const workspaceId = selected.workspace_id;
  const base = `/api/owners/${encodeURIComponent(owner)}/workspaces/${encodeURIComponent(workspaceId)}`;
  const [draft, setDraft] = useState<Workspace | null>(null);
  const [saved, setSaved] = useState<Workspace | null>(null);
  const [loadedScope, setLoadedScope] = useState("");
  const [conflict, setConflict] = useState<Workspace | null>(null);
  const [tab, setTab] = useState<Tab>("resources");
  const [collectionId, setCollectionId] = useState("");
  const [requestId, setRequestId] = useState("");
  const [saving, setSaving] = useState(false);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<ReturnType<typeof liveError>>("");
  const [runResult, setRunResult] = useState<RunResult | null>(null);
  const mounted = useRef(true);
  const scope = `${accountId}:${owner}:${workspaceId}:${selected.role}:${epoch}`;
  const latest = useRef(scope);
  latest.current = scope;
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  useEffect(() => { setDraft(null); setSaved(null); setLoadedScope(""); setConflict(null); setCollectionId(""); setRequestId(""); setTab("resources"); setError(""); setRunResult(null); setSaving(false); setRunning(false); }, [scope]);
  const current = (origin: string) => mounted.current && latest.current === origin && token() === credential;
  const workspace = useQuery({
    queryKey: ["shared-workspace", accountId, owner, workspaceId, selected.role, epoch],
    queryFn: () => api<Workspace>(base), enabled: !!credential, retry: false, refetchInterval: 3000,
  });
  const history = useQuery({ queryKey: ["shared-history", accountId, owner, workspaceId, epoch], queryFn: () => api<HistoryEntry[]>(`${base}/history`), enabled: !!credential && selected.role !== "viewer" && tab === "history" && !workspace.error, retry: false });
  const reports = useQuery({ queryKey: ["shared-reports", accountId, owner, workspaceId, epoch], queryFn: () => api<RunReportPage>(`${base}/reports`), enabled: !!credential && selected.role !== "viewer" && tab === "reports" && !workspace.error, retry: false });
  useEffect(() => {
    if (workspace.error || !workspace.data) { if (workspace.error) { setDraft(null); setSaved(null); setLoadedScope(""); setRunResult(null); } return; }
    if (!saved) { setDraft(workspace.data); setSaved(workspace.data); setLoadedScope(scope); return; }
    if (workspace.data.revision !== saved.revision) {
      if (hasChanges({ draft, saved })) setConflict(workspace.data);
      else { setDraft(workspace.data); setSaved(workspace.data); setConflict(null); }
    }
  }, [workspace.data, workspace.error, draft, saved, scope]);
  const canEdit = selected.role !== "viewer";
  const dirty = hasChanges({ draft, saved });
  useEffect(() => { onDirtyChange(dirty); }, [dirty, onDirtyChange]);
  useEffect(() => () => onDirtyChange(false), [onDirtyChange]);
  const collection = draft?.data.collections.find((item) => item.id === collectionId) ?? draft?.data.collections[0];
  const request = collection?.requests.find((item) => item.id === requestId) ?? collection?.requests[0];
  function updateRequest(change: (request: RequestSpec) => RequestSpec) {
    if (!canEdit || !collection || !request) return;
    setDraft((currentDraft) => currentDraft ? ({ ...currentDraft, data: { ...currentDraft.data, collections: currentDraft.data.collections.map((item) => item.id === collection.id ? { ...item, requests: item.requests.map((entry) => entry.id === request.id ? change(entry) : entry) } : item) } }) : null);
  }
  async function save() {
    if (!canEdit || !draft || !saved || !dirty || saving || conflict || !current(scope)) return;
    const origin = scope;
    const snapshot = structuredClone(draft);
    setSaving(true); setError("");
    try {
      const updated = await api<Workspace>(base, "PUT", { name: snapshot.name, data: snapshot.data, expected_revision: saved.revision });
      if (current(origin)) { setDraft((latestDraft) => latestDraft?.id === updated.id ? { ...updated, name: latestDraft.name, data: latestDraft.data } : latestDraft); setSaved(updated); setConflict(null); client.setQueryData(["shared-workspace", accountId, owner, workspaceId, selected.role, epoch], updated); void client.invalidateQueries({ queryKey: ["shared-workspaces", accountId] }); }
    } catch (cause) {
      if (current(origin)) { setError(liveError(cause)); try { const remote = await api<Workspace>(base); if (current(origin) && remote.revision !== saved.revision) setConflict(remote); } catch { /* Access revocation is shown by the next workspace refresh. */ } }
    } finally { if (current(origin)) setSaving(false); }
  }
  async function runCollection() {
    if (!canEdit || !collection || running || dirty || !current(scope)) return;
    const origin = scope;
    setRunning(true); setError(""); setRunResult(null);
    try {
      const result = await api<RunResult>(`${base}/run`, "POST", { collection_id: collection.id, job_id: id() });
      if (current(origin)) { setRunResult(result); void client.invalidateQueries({ queryKey: ["shared-history", accountId, owner, workspaceId] }); void client.invalidateQueries({ queryKey: ["shared-reports", accountId, owner, workspaceId] }); }
    } catch (cause) { if (current(origin)) setError(liveError(cause)); }
    finally { if (current(origin)) setRunning(false); }
  }
  return <main className="main-workspace shared-workspace" aria-label={t("共享工作区")}>
    <Flex align="center" justify="between" gap="3" wrap="wrap"><Flex direction="column" gap="1"><Heading size="5">{selected.workspace_name}</Heading><Text size="2" color="gray">{t("所有者")}: {selected.owner_username} · {selected.role === "viewer" ? t("查看者") : selected.role === "admin" ? t("管理员") : t("编辑者")}</Text></Flex><Button variant="soft" color="gray" onClick={onClose}>{t("返回我的工作区")}</Button></Flex>
    {selected.role === "viewer" && <Callout.Root color="gray"><Callout.Text>{t("此工作区为只读。不能保存、运行请求或查看私有执行记录。")}</Callout.Text></Callout.Root>}
    {workspace.isPending && <Text role="status">{t("正在加载共享工作区…")}</Text>}
    {workspace.error && <Callout.Root color="red" role="alert"><Callout.Text>{t("共享工作区访问已失效或暂时不可用。")}</Callout.Text></Callout.Root>}
    {error && <Callout.Root color="red" role="alert"><Callout.Text>{error}</Callout.Text></Callout.Root>}
    {!workspace.error && draft && loadedScope === scope && <Flex direction="column" gap="4">
      <Flex gap="2" wrap="wrap"><Button variant={tab === "resources" ? "solid" : "soft"} onClick={() => setTab("resources")}>{t("资源")}</Button>{canEdit && <><Button variant={tab === "history" ? "solid" : "soft"} onClick={() => setTab("history")}>{t("执行历史")}</Button><Button variant={tab === "reports" ? "solid" : "soft"} onClick={() => setTab("reports")}>{t("测试报告")}</Button></>}</Flex>
      {tab === "resources" && <>
        {canEdit ? <Field label={t("工作区名称")}><TextField.Root value={draft.name} onChange={(event) => setDraft((currentDraft) => currentDraft ? { ...currentDraft, name: event.target.value } : null)} /></Field> : <Text size="2">{draft.name}</Text>}
        {draft.data.collections.length === 0 ? <Text size="2" color="gray">{t("此工作区还没有集合")}</Text> : <>
          <Choice label={t("集合")} value={collection?.id ?? ""} onChange={(value) => { setCollectionId(value); setRequestId(""); }} options={draft.data.collections.map((item) => ({ value: item.id, label: item.name }))} />
          {collection && <Flex align="center" justify="between" gap="2" wrap="wrap"><Heading size="3">{collection.name}</Heading>{canEdit && <Button loading={running} disabled={running || dirty || collection.requests.length === 0} onClick={() => void runCollection()}>{t("运行集合")}</Button>}</Flex>}
          {canEdit && dirty && <Text size="1" color="gray">{t("先保存修改，再运行集合。")}</Text>}
          {runResult && <Text role="status" size="2">{t("通过")}: {runResult.passed} · {t("失败")}: {runResult.failed}</Text>}
          {collection && collection.requests.length > 0 && <><Choice label={t("请求")} value={request?.id ?? ""} onChange={setRequestId} options={collection.requests.map((item) => ({ value: item.id, label: `${item.method} ${item.name}` }))} />{request && (canEdit ? <Flex direction="column" gap="3"><Field label={t("请求名称")}><TextField.Root value={request.name} onChange={(event) => updateRequest((item) => ({ ...item, name: event.target.value }))} /></Field><Flex className="shared-request-line" gap="2" wrap="wrap"><Choice label={t("方法")} value={request.method} onChange={(value) => updateRequest((item) => ({ ...item, method: value }))} options={["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"].map((value) => ({ value, label: value }))} /><Field label={t("请求地址")}><TextField.Root value={request.url} onChange={(event) => updateRequest((item) => ({ ...item, url: event.target.value }))} /></Field></Flex><Field label={t("描述")}><TextArea value={request.description} onChange={(event) => updateRequest((item) => ({ ...item, description: event.target.value }))} /></Field></Flex> : <Flex direction="column" gap="1"><Text weight="medium">{request.method} {request.name}</Text><Text size="2" style={{ overflowWrap: "anywhere" }}>{request.url}</Text><Text size="2" color="gray">{request.description}</Text></Flex>)}</>}
        </>}
        {conflict && <Callout.Root color="amber" role="alert"><Callout.Text>{t("共享工作区已被其他成员更新。选择要保留的版本。")}</Callout.Text><Flex gap="2" wrap="wrap" mt="2"><Button variant="soft" onClick={() => { setDraft(conflict); setSaved(conflict); setConflict(null); setError(""); }}>{t("使用服务器版本")}</Button><Button variant="soft" onClick={() => { setSaved(conflict); setDraft((currentDraft) => currentDraft ? { ...currentDraft, revision: conflict.revision } : null); setConflict(null); setError(""); }}>{t("保留我的修改")}</Button></Flex></Callout.Root>}
        {canEdit && <Button loading={saving} disabled={!dirty || saving || !!conflict} onClick={() => void save()}>{t("保存共享工作区")}</Button>}
      </>}
      {tab === "history" && canEdit && <Flex direction="column" gap="2">{history.isPending && <Text role="status">{t("正在加载执行历史…")}</Text>}{history.error && <Callout.Root color="red" role="alert"><Callout.Text>{liveError(history.error)}</Callout.Text></Callout.Root>}{!history.error && history.data?.length === 0 && <Text size="2" color="gray">{t("暂无执行历史")}</Text>}{!history.error && history.data?.map((item) => <Flex key={item.id} justify="between" gap="2" wrap="wrap"><Text size="2">{item.method} {item.request_name}</Text><Text size="2">{item.status} · {item.created_at}</Text></Flex>)}</Flex>}
      {tab === "reports" && canEdit && <Flex direction="column" gap="2">{reports.isPending && <Text role="status">{t("正在加载测试报告…")}</Text>}{reports.error && <Callout.Root color="red" role="alert"><Callout.Text>{liveError(reports.error)}</Callout.Text></Callout.Root>}{!reports.error && reports.data?.items.length === 0 && <Text size="2" color="gray">{t("暂无测试报告")}</Text>}{!reports.error && reports.data?.items.map((item) => <Flex key={item.id} justify="between" gap="2" wrap="wrap"><Text size="2">{item.collection_name}</Text><Text size="2">{t("通过")}: {item.summary.passed} · {t("失败")}: {item.summary.failed}</Text></Flex>)}</Flex>}
    </Flex>}
  </main>;
}
