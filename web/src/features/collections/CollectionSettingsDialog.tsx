import { useState } from "react";
import { Button, Callout, Dialog, Flex } from "@radix-ui/themes";
import { t, useLanguage, translateCopy } from "../../shared/i18n";
import { errorCopy, LocalizedError } from "../../shared/i18n/errors";
import { id } from "../../shared/model";
import type { Collection } from "../../shared/types";
import { useWorkbench } from "../workbench/context";
import CollectionSettingsFields, { type CollectionSettingsTarget } from "./CollectionSettingsFields";
export type { CollectionSettingsTarget } from "./CollectionSettingsFields";
import { moveCollection } from "./tree";
export default function CollectionSettingsDialog({ target, close }: { target: CollectionSettingsTarget; close: () => void }) {
  useLanguage(); const state = useWorkbench();
  const [owner] = useState({ workspace: state.draft?.id, account: state.accountId });
  const [draft, setDraft] = useState<Collection>(() => target.kind === "edit"
    ? structuredClone(state.draft!.data.collections.find(c => c.id === target.id)!)
    : { id: id(), parent_id: target.parent_id, name: t("新建目录"), description: "", requests: [] });
  const [original] = useState(()=>target.kind==="edit"?JSON.stringify({...state.draft!.data.collections.find(c=>c.id===target.id)!,requests:[]}):null);
  const [error, setError] = useState<ReturnType<typeof errorCopy> | null>(null);
  const update = (patch: Partial<Collection>) => setDraft(current => ({ ...current, ...patch }));
  const apply = () => {
    if (state.accountRef.current !== owner.account || state.stateRef.current.draft?.id !== owner.workspace) return close();
    if (!draft.name.trim()) return;
    try {
      state.updateData(data => {
        if(target.kind==="edit") {
          const current=data.collections.find(c=>c.id===draft.id);
          if(!current || JSON.stringify({...current,requests:[]})!==original) throw new LocalizedError("目录设置在编辑期间已变更，请重新打开设置");
        }
        const candidate = target.kind === "create"
          ? { ...data, collections: [...data.collections, draft] }
          : { ...data, collections: data.collections.map(c => c.id === draft.id ? { ...draft, requests: c.requests } : c) };
        const next=moveCollection(candidate, draft.id, draft.parent_id ?? null);
        const before=data.collections.find(c=>c.id===draft.id)?.variables ?? [];
        if(!state.localVariables.reconcile("collection",draft.id,before,draft.variables??[])) throw new LocalizedError("本地变量更新未完成，请重试");
        return next;
      });
      close();
    } catch (e) { setError(errorCopy(e)); }
  };
  return <Dialog.Root open onOpenChange={open => { if (!open) close(); }}><Dialog.Content maxWidth="min(820px, calc(100vw - 24px))" className="collection-settings-dialog" onKeyDown={event => { if (event.ctrlKey || event.metaKey) event.stopPropagation(); }} onCloseAutoFocus={event => {
    event.preventDefault(); const returnId = target.kind === "edit" ? target.id : target.parent_id;
    const button = [...document.querySelectorAll<HTMLButtonElement>("[data-collection-actions]")].find(el => el.dataset.collectionActions === returnId);
    button?.focus();
  }}>
    <Dialog.Title>{target.kind === "create" ? t("新建目录") : t("集合与目录设置")}</Dialog.Title>
    <Dialog.Description>{t("父级变量和鉴权在执行时继承；保存工作区后同步这些设置。")}</Dialog.Description>
    <Flex direction="column" gap="3" mt="4">
      <CollectionSettingsFields collection={draft} collections={state.draft?.data.collections ?? []} dark={state.dark} onChange={update} />
      {error && <Callout.Root color="red" role="alert"><Callout.Text>{translateCopy(error)}</Callout.Text></Callout.Root>}
      <Flex justify="end" gap="3"><Dialog.Close><Button variant="soft" color="gray">{t("取消")}</Button></Dialog.Close><Button disabled={!draft.name.trim()} onClick={apply}>{t("应用")}</Button></Flex>
    </Flex>
  </Dialog.Content></Dialog.Root>;
}
