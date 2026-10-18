import { useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { AlertDialog, Button, Callout, Dialog, Flex, Heading, ScrollArea, Text } from "@radix-ui/themes";
import { toast } from "sonner";
import { api, ApiError } from "../../shared/api";
import { t, useLanguage, message, liveTranslation, translateCopy } from "../../shared/i18n";
import { errorCopy, liveError } from "../../shared/i18n/errors";
import type { ErrorCopy } from "../../shared/i18n/errors";
import type { Workspace, RequestSpec } from "../../shared/types";
import { Choice } from "../../shared/ui";
import { useWorkbench } from "../workbench/context";

type VersionSummary = {
  revision: number;
  updated_at: string;
  collection_id: string | null;
  name: string | null;
  method: string | null;
  exists: boolean;
};
type VersionPage = { versions: VersionSummary[]; next_before: number | null };
type VersionDetail = {
  revision: number;
  updated_at: string;
  collection_id: string | null;
  request: RequestSpec | null;
};

export default function RequestVersionDialog() {
  const { language } = useLanguage();
  const client = useQueryClient();
  const state = useWorkbench();
  const requestId = state.versionRequestId;
  const workspace = state.saved;
  const account = state.accountId;
  const origin = JSON.stringify([account, workspace?.id ?? null, requestId]);
  const originRef = useRef(origin);
  originRef.current = origin;
  const [pages, setPages] = useState<VersionPage[]>([]);
  const [selectedRevision, setSelectedRevision] = useState<number | null>(null);
  const [detail, setDetail] = useState<VersionDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const [detailLoading, setDetailLoading] = useState(false);
  const [problem, setProblem] = useState<ErrorCopy>("");
  const [confirming, setConfirming] = useState(false);
  const [restoring, setRestoring] = useState(false);
  const [destination, setDestination] = useState("");
  const base = workspace && requestId
    ? `/api/workspaces/${encodeURIComponent(workspace.id)}/requests/${encodeURIComponent(requestId)}/versions`
    : "";
  const versions = pages.flatMap((page) => page.versions);
  const nextBefore = pages.at(-1)?.next_before;
  const currentRequest = workspace?.data.collections.flatMap((collection) => collection.requests)
    .find((request) => request.id === requestId);
  const historicalCollectionExists = !!detail?.collection_id && !!workspace?.data.collections
    .some((collection) => collection.id === detail.collection_id);
  const needsDestination = !currentRequest && !!detail?.request && !historicalCollectionExists;

  useEffect(() => {
    setPages([]);
    setSelectedRevision(null);
    setDetail(null);
    setProblem("");
    if (!base) return;
    let active = true;
    setLoading(true);
    void api<VersionPage>(base).then((page) => {
      if (!active) return;
      setPages([page]);
      setSelectedRevision(page.versions[0]?.revision ?? null);
    }).catch((error) => { if (active) setProblem(errorCopy(error)); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [base, account]);

  useEffect(() => {
    setDetail(null);
    if (!base || selectedRevision === null) return;
    let active = true;
    setDetailLoading(true);
    void api<VersionDetail>(`${base}/${selectedRevision}`).then((value) => {
      if (active) setDetail(value);
    }).catch((error) => { if (active) setProblem(errorCopy(error)); })
      .finally(() => { if (active) setDetailLoading(false); });
    return () => { active = false; };
  }, [base, selectedRevision, account]);

  useEffect(() => {
    if (!workspace || !needsDestination) {
      setDestination("");
      return;
    }
    setDestination(workspace.data.collections[0]?.id ?? "");
  }, [workspace, needsDestination]);

  async function loadMore() {
    if (!base || !workspace || !requestId || nextBefore == null || loading) return;
    const source = origin;
    setLoading(true);
    setProblem("");
    try {
      const page = await api<VersionPage>(`${base}?before=${nextBefore}`);
      if (originRef.current !== source) return;
      setPages((previous) => [...previous, page]);
      setSelectedRevision((previous) => previous ?? page.versions[0]?.revision ?? null);
    } catch (error) {
      if (originRef.current === source) setProblem(errorCopy(error));
    } finally { if (originRef.current === source) setLoading(false); }
  }

  async function applyRestore(revision: number, sourceAccount: string, sourceWorkspace: string, sourceRequest: string, selectedDestination: string | null, historicalCollectionId: string | null, targetExists: boolean) {
    const current = state.stateRef.current.saved;
    if (state.accountRef.current !== sourceAccount || current?.id !== sourceWorkspace) return;
    const liveRequest = current.data.collections.some((collection) => collection.requests.some((request) => request.id === sourceRequest));
    const historicalExists = current.data.collections.some((collection) => collection.id === historicalCollectionId);
    const requiresDestination = targetExists && !liveRequest && !historicalExists;
    const destinationId = requiresDestination && selectedDestination && current.data.collections.some((collection) => collection.id === selectedDestination)
      ? selectedDestination : null;
    if (requiresDestination && !destinationId) {
      toast.error(liveTranslation("目标集合已变更，请重新选择后恢复。"));
      return;
    }
    if (!targetExists && !liveRequest) {
      toast.error(liveTranslation("当前请求已不存在。"));
      return;
    }
    setRestoring(true);
    try {
      const restored = await api<Workspace>(
        `/api/workspaces/${encodeURIComponent(sourceWorkspace)}/requests/${encodeURIComponent(sourceRequest)}/versions/${revision}/restore`,
        "POST",
        { expected_revision: current.revision, ...(destinationId ? { destination_collection_id: destinationId } : {}) },
      );
      if (state.accountRef.current !== sourceAccount || state.stateRef.current.saved?.id !== sourceWorkspace) return;
      state.installWorkspace(restored);
      state.setResponse();
      const requests = restored.data.collections.flatMap((collection) => collection.requests);
      state.setRequestId(requests.some((request) => request.id === sourceRequest)
        ? sourceRequest : (requests[0]?.id ?? ""));
      state.setVersionRequestId(null);
      void client.invalidateQueries({ queryKey: ["workspace-versions", sourceAccount, sourceWorkspace] });
      toast.success(liveTranslation("已恢复请求版本"));
    } catch (error) {
      if (state.accountRef.current !== sourceAccount || state.stateRef.current.saved?.id !== sourceWorkspace) return;
      if (error instanceof ApiError && error.status === 409) {
        try {
          const remote = await api<Workspace>(`/api/workspaces/${encodeURIComponent(sourceWorkspace)}`);
          if (state.accountRef.current === sourceAccount && state.stateRef.current.saved?.id === sourceWorkspace && remote.revision !== current.revision)
            state.setSaveConflict(remote);
        } catch (fetchError) { toast.error(liveError(fetchError)); }
      }
      toast.error(liveError(error));
    } finally { setRestoring(false); setConfirming(false); }
  }

  function requestRestore() {
    if (!workspace || !requestId || !detail || restoring || detailLoading) return;
    if (!currentRequest && !detail.request) return;
    if (needsDestination && !destination) return;
    const revision = detail.revision;
    const sourceAccount = account;
    const sourceWorkspace = workspace.id;
    const sourceRequest = requestId;
    const destinationId = needsDestination ? destination : null;
    const historicalCollectionId = detail.collection_id;
    const targetExists = !!detail.request;
    setConfirming(false);
    if (state.dirty) {
      state.setGuard({
        title: message("恢复请求版本"),
        description: message("当前工作区有未保存修改。可以保存后恢复请求，或放弃修改后恢复。"),
        saveFirst: true,
        action: () => applyRestore(revision, sourceAccount, sourceWorkspace, sourceRequest, destinationId, historicalCollectionId, targetExists),
      });
    } else void applyRestore(revision, sourceAccount, sourceWorkspace, sourceRequest, destinationId, historicalCollectionId, targetExists);
  }

  return <Dialog.Root open={!!requestId && !!workspace} onOpenChange={(open) => { if (!open && !restoring) state.setVersionRequestId(null); }}>
    <Dialog.Content maxWidth="660px">
      <Dialog.Title>{t("请求版本历史")}</Dialog.Title>
      <Dialog.Description>{t("按请求内容变更查看旧版本；恢复只修改这个请求。")}</Dialog.Description>
      <Flex direction="column" gap="4" mt="4">
        {problem && <Callout.Root color="red" role="alert"><Callout.Text>{translateCopy(problem)}</Callout.Text></Callout.Root>}
        {loading && pages.length === 0 && <Text role="status">{t("正在加载请求历史…")}</Text>}
        {!loading && pages.length > 0 && versions.length === 0 && nextBefore == null && <Text color="gray">{t("这个请求还没有内容变更记录。")}</Text>}
        <Flex gap="4" wrap="wrap">
          {(versions.length > 0 || nextBefore != null) && <ScrollArea style={{ maxHeight: 320, minWidth: 180, flex: "1 1 180px" }}>
            <Flex direction="column" gap="2">
              {versions.map((version) => <Button key={version.revision} type="button" color="gray" highContrast variant={version.revision === selectedRevision ? "solid" : "soft"} onClick={() => setSelectedRevision(version.revision)} aria-pressed={version.revision === selectedRevision}>
                {t("版本 {{revision}}", { revision: version.revision })} · {version.exists ? version.name : t("请求不存在")}
              </Button>)}
              {nextBefore != null && <Button type="button" variant="ghost" color="gray" loading={loading} onClick={() => void loadMore()}>{t("加载更早版本")}</Button>}
            </Flex>
          </ScrollArea>}
          {selectedRevision !== null && <Flex direction="column" gap="2" style={{ minWidth: 200, flex: "2 1 200px", overflowWrap: "anywhere" }}>
            <Heading size="3">{t("版本 {{revision}}", { revision: selectedRevision })}</Heading>
            {detailLoading && <Text role="status">{t("正在加载版本详情…")}</Text>}
            {detail && <>
              <Text size="2" color="gray">{new Date(detail.updated_at).toLocaleString(language)}</Text>
              {detail.request ? <>
                <Text size="2" weight="medium">{detail.request.name}</Text>
                <Text size="2">{detail.request.method} {detail.request.url}</Text>
                {detail.request.description && <Text size="2">{detail.request.description}</Text>}
                {detail.request.body && <ScrollArea style={{ maxHeight: 110 }}><pre style={{ margin: 0, whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>{detail.request.body.slice(0, 2000)}{detail.request.body.length > 2000 ? "…" : ""}</pre></ScrollArea>}
              </> : <Text color="gray">{t("此版本中请求不存在；恢复将删除当前请求。")}</Text>}
              {needsDestination && workspace && workspace.data.collections.length > 0 && <Choice value={destination} onChange={setDestination} label={t("恢复到集合")} options={workspace.data.collections.map((collection) => ({ value: collection.id, label: collection.name }))} />}
              {needsDestination && workspace?.data.collections.length === 0 && <Text color="red">{t("先创建集合，再恢复已删除的请求。")}</Text>}
              <AlertDialog.Root open={confirming} onOpenChange={setConfirming}>
                <AlertDialog.Trigger><Button type="button" color="gray" highContrast disabled={restoring || (needsDestination && !destination) || (!currentRequest && !detail.request)}>{t("恢复此请求版本")}</Button></AlertDialog.Trigger>
                <AlertDialog.Content maxWidth="440px">
                  <AlertDialog.Title>{t("恢复请求版本")}</AlertDialog.Title>
                  <AlertDialog.Description>{detail.request
                    ? t("将版本 {{revision}} 的请求恢复为新的当前版本。其他资源保持当前内容。", { revision: detail.revision })
                    : t("版本 {{revision}} 中没有此请求。恢复将删除当前请求。", { revision: detail.revision })}</AlertDialog.Description>
                  <Flex justify="end" gap="3" mt="5"><AlertDialog.Cancel><Button type="button" color="gray" variant="soft">{t("取消")}</Button></AlertDialog.Cancel><Button type="button" color="red" loading={restoring} onClick={requestRestore}>{t("确认恢复")}</Button></Flex>
                </AlertDialog.Content>
              </AlertDialog.Root>
            </>}
          </Flex>}
        </Flex>
      </Flex>
    </Dialog.Content>
  </Dialog.Root>;
}
