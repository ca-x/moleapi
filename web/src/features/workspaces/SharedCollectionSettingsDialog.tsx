import { useState } from "react";
import { Button, Callout, Dialog, Flex } from "@radix-ui/themes";
import { t, translateCopy, useLanguage } from "../../shared/i18n";
import { errorCopy, LocalizedError } from "../../shared/i18n/errors";
import { id } from "../../shared/model";
import type { Collection, WorkspaceData } from "../../shared/types";
import CollectionSettingsFields, { type CollectionSettingsTarget } from "../collections/CollectionSettingsFields";
import { moveCollection } from "../collections/tree";

const metadata = (collection: Collection) => JSON.stringify({ ...collection, requests: [] });

export default function SharedCollectionSettingsDialog({ target, data, dark, onApply, close }: { target: CollectionSettingsTarget; data: WorkspaceData; dark: boolean; onApply: (data: WorkspaceData, collectionId: string) => void; close: () => void }) {
  useLanguage();
  const [draft, setDraft] = useState<Collection>(() => target.kind === "edit"
    ? structuredClone(data.collections.find((collection) => collection.id === target.id)!)
    : { id: id(), parent_id: target.parent_id, name: target.parent_id ? t("新建目录") : t("新建集合"), description: "", requests: [] });
  const [original] = useState(() => target.kind === "edit" ? metadata(draft) : null);
  const [error, setError] = useState<ReturnType<typeof errorCopy> | null>(null);
  function apply() {
    if (!draft.name.trim()) return;
    try {
      if (target.kind === "edit") {
        const current = data.collections.find((collection) => collection.id === draft.id);
        if (!current || metadata(current) !== original) throw new LocalizedError("目录设置在编辑期间已变更，请重新打开设置");
      }
      const shared = { ...draft, variables: draft.variables?.map((row) => { const next = { ...row }; delete next.local_value; return next; }) };
      const candidate = target.kind === "create"
        ? { ...data, collections: [...data.collections, shared] }
        : { ...data, collections: data.collections.map((collection) => collection.id === shared.id ? { ...shared, requests: collection.requests } : collection) };
      onApply(moveCollection(candidate, shared.id, shared.parent_id ?? null), shared.id);
      close();
    } catch (cause) { setError(errorCopy(cause)); }
  }
  return <Dialog.Root open onOpenChange={(open) => { if (!open) close(); }}><Dialog.Content maxWidth="min(820px, calc(100vw - 24px))" className="collection-settings-dialog" onKeyDown={(event) => { if (event.ctrlKey || event.metaKey) event.stopPropagation(); }}>
    <Dialog.Title>{target.kind === "create" ? target.parent_id ? t("新建目录") : t("新建集合") : t("集合与目录设置")}</Dialog.Title>
    <Dialog.Description>{t("父级变量和鉴权在执行时继承；保存工作区后同步这些设置。")}</Dialog.Description>
    <Flex direction="column" gap="3" mt="4">
      <CollectionSettingsFields collection={draft} collections={data.collections} dark={dark} tokenManagement={false} onChange={(patch) => setDraft((current) => ({ ...current, ...patch }))} />
      {error && <Callout.Root color="red" role="alert"><Callout.Text>{translateCopy(error)}</Callout.Text></Callout.Root>}
      <Flex justify="end" gap="3"><Dialog.Close><Button variant="soft" color="gray">{t("取消")}</Button></Dialog.Close><Button disabled={!draft.name.trim()} onClick={apply}>{t("应用")}</Button></Flex>
    </Flex>
  </Dialog.Content></Dialog.Root>;
}
