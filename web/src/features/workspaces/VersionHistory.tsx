import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertDialog, Button, Callout, Flex, Heading, ScrollArea, Text, TextField } from "@radix-ui/themes";
import { toast } from "sonner";
import { api, ApiError } from "../../shared/api";
import { t, useLanguage, message, liveTranslation } from "../../shared/i18n";
import { liveError } from "../../shared/i18n/errors";
import type { Workspace } from "../../shared/types";
import { useWorkbench } from "../workbench/context";

export default function VersionHistory() {
  const { language } = useLanguage();
  const client = useQueryClient();
  const state = useWorkbench();
  const workspace = state.saved;
  const account = state.accountId;
  const [selectedRevision, setSelectedRevision] = useState<number | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [requestSearch, setRequestSearch] = useState("");
  const history = useQuery({
    queryKey: ["workspace-versions", account, workspace?.id],
    queryFn: () => api<Workspace[]>(`/api/workspaces/${encodeURIComponent(workspace!.id)}/versions`),
    enabled: !!workspace && state.modal === "versions",
  });
  const selected = history.data?.find((item) => item.revision === selectedRevision)
    ?? history.data?.[0];
  const needle = requestSearch.trim().toLocaleLowerCase();
  const matchingRequests = selected?.data.collections.flatMap((collection) =>
    collection.requests.map((request) => ({ collection, request })))
    .filter(({ collection, request }) => !needle || [collection.name, request.name, request.id, request.method]
      .some((value) => value.toLocaleLowerCase().includes(needle))) ?? [];

  async function applyRestore(revision: number, origin: string, originAccount: string) {
    const current = state.stateRef.current.saved;
    if (!current || current.id !== origin || state.accountRef.current !== originAccount) return;
    setBusy(true);
    try {
      const restored = await api<Workspace>(
        `/api/workspaces/${encodeURIComponent(origin)}/versions/${revision}/restore`,
        "POST",
        { expected_revision: current.revision },
      );
      if (state.accountRef.current !== originAccount || state.stateRef.current.saved?.id !== origin) return;
      state.installWorkspace(restored);
      state.setResponse();
      const requests = restored.data.collections.flatMap((collection) => collection.requests);
      state.setRequestId(requests.some((request) => request.id === state.requestId)
        ? state.requestId : (requests[0]?.id ?? ""));
      void client.invalidateQueries({ queryKey: ["workspace-versions", originAccount, origin] });
      state.setModal(null);
      toast.success(liveTranslation("已恢复工作区版本"));
    } catch (error) {
      if (state.accountRef.current !== originAccount || state.stateRef.current.saved?.id !== origin) return;
      if (error instanceof ApiError && error.status === 409 && state.accountRef.current === originAccount) {
        try {
          const remote = await api<Workspace>(`/api/workspaces/${encodeURIComponent(origin)}`);
          if (state.accountRef.current === originAccount && state.stateRef.current.saved?.id === origin)
            state.setSaveConflict(remote);
        } catch (fetchError) { toast.error(liveError(fetchError)); }
      }
      toast.error(liveError(error));
    } finally {
      setBusy(false);
      setConfirming(false);
    }
  }

  function requestRestore() {
    if (!selected || !workspace || busy) return;
    const revision = selected.revision;
    const origin = workspace.id;
    const originAccount = account;
    setConfirming(false);
    if (state.dirty) {
      state.setGuard({
        title: message("恢复工作区版本"),
        description: message("当前工作区有未保存修改。可以保存后恢复，或放弃修改后恢复。"),
        saveFirst: true,
        action: () => applyRestore(revision, origin, originAccount),
      });
    } else void applyRestore(revision, origin, originAccount);
  }

  return (
    <Flex direction="column" gap="4" mt="4">
      {history.isPending && <Text role="status">{t("正在加载历史版本…")}</Text>}
      {history.error && <Callout.Root color="red" role="alert"><Callout.Text>{liveError(history.error)}</Callout.Text></Callout.Root>}
      {history.data?.length === 0 && <Text color="gray">{t("还没有历史版本。保存修改后会产生版本记录。")}</Text>}
      {!!history.data?.length && <Flex gap="4" wrap="wrap">
        <ScrollArea style={{ maxHeight: 340, minWidth: 170, flex: "1 1 170px" }}>
          <Flex direction="column" gap="2">
            {history.data.map((item) => <Button key={item.revision} type="button" color="gray" highContrast variant={item.revision === selected?.revision ? "solid" : "soft"} onClick={() => setSelectedRevision(item.revision)} aria-pressed={item.revision === selected?.revision}>
              {t("版本 {{revision}}", { revision: item.revision })}
            </Button>)}
          </Flex>
        </ScrollArea>
        {selected && <Flex direction="column" gap="2" style={{ minWidth: 200, flex: "2 1 200px" }}>
          <Heading size="3">{t("版本 {{revision}}", { revision: selected.revision })}</Heading>
          <Text size="2" color="gray">{new Date(selected.updated_at).toLocaleString(language)}</Text>
          <Text size="2">{t("工作区名称：{{name}}", { name: selected.name })}</Text>
          <Text size="2">{t("集合 {{collections}} 个，请求 {{requests}} 个", { collections: selected.data.collections.length, requests: selected.data.collections.reduce((count, collection) => count + collection.requests.length, 0) })}</Text>
          <TextField.Root aria-label={t("查找历史请求")} placeholder={t("按请求名称或 ID 查找")} value={requestSearch} onChange={(event) => setRequestSearch(event.target.value)} />
          <ScrollArea style={{ maxHeight: 160 }}>
            <ul style={{ margin: 0, paddingInlineStart: 20, overflowWrap: "anywhere" }}>
              {matchingRequests.slice(0, 20).map(({ collection, request }) => <li key={request.id}>
                <Text size="2" color="gray">{collection.name} · </Text>
                <Button size="1" variant="ghost" color="gray" onClick={() => { state.setModal(null); state.setVersionRequestId(request.id); }}>{request.name}</Button>
              </li>)}
              {matchingRequests.length > 20 && <li><Text size="2" color="gray">{t("匹配结果较多，请缩小搜索范围。")}</Text></li>}
              {matchingRequests.length === 0 && <li><Text size="2" color="gray">{t("没有匹配的历史请求。")}</Text></li>}
            </ul>
          </ScrollArea>
          <AlertDialog.Root open={confirming} onOpenChange={setConfirming}>
            <AlertDialog.Trigger><Button type="button" color="gray" highContrast disabled={busy}>{t("恢复此版本")}</Button></AlertDialog.Trigger>
            <AlertDialog.Content maxWidth="430px">
              <AlertDialog.Title>{t("恢复工作区版本")}</AlertDialog.Title>
              <AlertDialog.Description>{t("将版本 {{revision}} 恢复为新的当前版本。现有版本会保留在历史记录中。", { revision: selected.revision })}</AlertDialog.Description>
              <Flex justify="end" gap="3" mt="5">
                <AlertDialog.Cancel><Button type="button" variant="soft" color="gray">{t("取消")}</Button></AlertDialog.Cancel>
                <Button type="button" color="red" loading={busy} onClick={requestRestore}>{t("确认恢复")}</Button>
              </Flex>
            </AlertDialog.Content>
          </AlertDialog.Root>
        </Flex>}
      </Flex>}
    </Flex>
  );
}
