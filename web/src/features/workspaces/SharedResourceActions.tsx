import { useEffect, useState } from "react";
import { AlertDialog, Button, Callout, Flex, Text } from "@radix-ui/themes";
import { t, useLanguage } from "../../shared/i18n";
import { liveError } from "../../shared/i18n/errors";
import { id } from "../../shared/model";
import { Choice, Field } from "../../shared/ui";
import type { WorkspaceData } from "../../shared/types";
import { duplicateRequest, moveRequest, removalImpact, removeResource } from "./resourceActions";

type Action = "collection" | "request" | "move";
export default function SharedResourceActions({ data, workspaceName, collectionId, requestId, onChange }: { data: WorkspaceData; workspaceName: string; collectionId: string; requestId?: string; onChange: (data: WorkspaceData, collectionId: string, requestId: string) => void }) {
  useLanguage();
  const [action, setAction] = useState<{ kind: Action; data: WorkspaceData; workspaceName: string; collectionId: string; requestId?: string; name: string } | null>(null);
  const [destination, setDestination] = useState("");
  const [error, setError] = useState<ReturnType<typeof liveError>>("");
  useEffect(() => { setAction(null); setDestination(""); setError(""); }, [data, workspaceName, collectionId, requestId]);
  const collection = data.collections.find((item) => item.id === collectionId);
  const request = collection?.requests.find((item) => item.id === requestId);
  if (!collection) return null;
  const selected = action?.data === data && action.workspaceName === workspaceName && action.collectionId === collectionId && action.requestId === requestId && (action.kind === "collection" || request) ? action.kind : null;
  const target = action?.kind === "collection" ? { kind: "collection" as const, id: action.collectionId } : action?.requestId ? { kind: "request" as const, id: action.requestId } : null;
  const impact = selected && selected !== "move" && target ? removalImpact(data, target) : null;
  function confirm() {
    if (!selected || action?.data !== data || !collection) return;
    try {
      if (selected === "move" && action.requestId) onChange(moveRequest(data, action.requestId, destination), destination, action.requestId);
      else if (target && !removalImpact(data, target).dependencies.length) {
        const next = removeResource(data, target);
        const nextCollection = selected === "collection" ? next.collections[0]?.id ?? "" : collection.id;
        onChange(next, nextCollection, "");
      }
      setAction(null);
    } catch (cause) { setError(liveError(cause)); }
  }
  function open(kind: Action) { if (!collection) return; setError(""); setDestination(""); setAction({ kind, data, workspaceName, collectionId, requestId, name: kind === "collection" ? collection.name : request?.name ?? "" }); }
  return <>
    <Flex gap="2" wrap="wrap">
      {request && <><Button variant="outline" onClick={() => { const copyId = id(); onChange(duplicateRequest(data, request.id, copyId, t("{{value0}} 副本", { value0: request.name })), collection.id, copyId); }}>{t("复制请求")}</Button><Button variant="outline" disabled={data.collections.length < 2} onClick={() => open("move")}>{t("移动请求")}</Button><Button variant="outline" color="red" onClick={() => open("request")}>{t("删除请求")}</Button></>}
      <Button variant="outline" color="red" onClick={() => open("collection")}>{t("删除集合")}</Button>
    </Flex>
    <AlertDialog.Root open={!!selected} onOpenChange={(open) => { if (!open) setAction(null); }}><AlertDialog.Content maxWidth="min(620px, calc(100vw - 24px))" style={{ maxHeight: "calc(100dvh - 24px)", overflowY: "auto" }}>
      <AlertDialog.Title>{selected === "move" ? t("移动请求") : selected === "collection" ? t("删除集合") : t("删除请求")}</AlertDialog.Title>
      <AlertDialog.Description>{selected === "move" ? t("将「{{value0}}」移动到所选集合？保存后生效。", { value0: action?.name ?? "" }) : selected === "collection" ? t("删除「{{value0}}」及其所有子目录和请求。保存后生效。", { value0: action?.name ?? "" }) : t("删除「{{value0}}」，保存后生效。", { value0: action?.name ?? "" })}</AlertDialog.Description>
      {selected === "move" && <Field label={t("目标集合")}><Choice label={t("目标集合")} value={destination} onChange={(value) => { setDestination(value); setError(""); }} options={data.collections.filter((item) => item.id !== collection.id).map((item) => ({ value: item.id, label: item.name }))} /></Field>}
      {selected === "collection" && impact && <Text as="p" size="2">{t("删除范围：{{collections}} 个集合，{{requests}} 个请求；{{documents}} 篇文档移至工作区根目录。", { collections: impact.collectionCount, requests: impact.requestCount, documents: impact.documentCount })}</Text>}
      {!!impact?.dependencies.length && <Callout.Root color="amber" style={{ overflowWrap: "anywhere" }}><Callout.Text>{t("所选资源仍被场景或数据集引用。请先更新引用，再删除。")}</Callout.Text><ul>{impact.dependencies.slice(0, 20).map((item) => <li key={`${item.kind}/${item.id}`}>{item.kind === "scenario" ? t("场景") : t("保存的数据集")}: {item.name} <code>{item.id}</code></li>)}</ul>{impact.dependencies.length > 20 && <Text size="2">{t("另有 {{count}} 项。", { count: impact.dependencies.length - 20 })}</Text>}</Callout.Root>}
      {error && <Callout.Root color="red" role="alert"><Callout.Text>{error}</Callout.Text></Callout.Root>}
      <Flex justify="end" gap="3" mt="4"><AlertDialog.Cancel><Button variant="soft" color="gray">{t("取消")}</Button></AlertDialog.Cancel><Button color={selected === "move" ? undefined : "red"} disabled={selected === "move" ? !destination : !!impact?.dependencies.length} onClick={confirm}>{selected === "move" ? t("确认移动") : t("确认删除")}</Button></Flex>
    </AlertDialog.Content></AlertDialog.Root>
  </>;
}
