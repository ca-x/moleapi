import { useState } from "react";
import { Button, Callout, Checkbox, Dialog, Flex, Tabs, Text, TextField } from "@radix-ui/themes";
import { Editor, Field, Choice, PairEditor } from "../../shared/ui";
import { t, useLanguage, translateCopy } from "../../shared/i18n";
import { errorCopy, LocalizedError } from "../../shared/i18n/errors";
import { id } from "../../shared/model";
import type { Auth, Collection } from "../../shared/types";
import { useWorkbench } from "../workbench/context";
import RequestAuthEditor from "../authentication/RequestAuthEditor";
import { collectionDescendants, moveCollection } from "./tree";
export type CollectionSettingsTarget = { kind: "edit"; id: string } | { kind: "create"; parent_id: string };
const inherited = (): Auth => ({ kind: "inherit", token: "", username: "", password: "" });
export default function CollectionSettingsDialog({ target, close }: { target: CollectionSettingsTarget; close: () => void }) {
  useLanguage(); const state = useWorkbench();
  const [owner] = useState({ workspace: state.draft?.id, account: state.accountId });
  const [draft, setDraft] = useState<Collection>(() => target.kind === "edit"
    ? structuredClone(state.draft!.data.collections.find(c => c.id === target.id)!)
    : { id: id(), parent_id: target.parent_id, name: t("新建目录"), description: "", requests: [] });
  const [original] = useState(()=>target.kind==="edit"?JSON.stringify({...state.draft!.data.collections.find(c=>c.id===target.id)!,requests:[]}):null);
  const [error, setError] = useState<ReturnType<typeof errorCopy> | null>(null);
  const excluded = collectionDescendants(state.draft?.data.collections ?? [], draft.id);
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
      <Field label={t("名称")}><TextField.Root value={draft.name} maxLength={256} onChange={e => update({ name: e.target.value })}/></Field>
      <Choice label={t("父目录")} value={draft.parent_id ? `collection:${draft.parent_id}` : "root"} options={[{ value: "root", label: t("工作区根目录") }, ...(state.draft?.data.collections ?? []).filter(c => !excluded.has(c.id)).map(c => ({ value: `collection:${c.id}`, label: c.name }))]} onChange={parent => update({ parent_id: parent === "root" ? null : parent.slice("collection:".length) })}/>
      <Tabs.Root defaultValue="auth"><Tabs.List><Tabs.Trigger value="auth">{t("鉴权")}</Tabs.Trigger><Tabs.Trigger value="variables">{t("变量")}</Tabs.Trigger><Tabs.Trigger value="scripts">{t("脚本")}</Tabs.Trigger></Tabs.List>
        <Tabs.Content value="auth"><RequestAuthEditor auth={draft.auth ?? inherited()} change={auth => update({ auth })} dark={state.dark}/></Tabs.Content>
        <Tabs.Content value="variables"><Text as="label" size="2"><Flex gap="2" align="center"><Checkbox checked={draft.variables_enabled!==false} onCheckedChange={value=>update({variables_enabled:value===true})}/>{t("执行目录变量")}</Flex></Text>{draft.variables_enabled===false && <Text as="p" size="1" color="gray">{t("目录变量仅保留源定义；启用后由 MoleAPI 在请求中解析。")}</Text>}<PairEditor rows={draft.variables ?? []} onChange={variables => update({ variables })} secrets keyLabel={t("变量名")} valueLabel={t("共享值")}/><Text size="1" color="gray">{t("同名变量按父级到子级覆盖，环境变量优先于集合变量。")}</Text></Tabs.Content>
        <Tabs.Content value="scripts"><Field label={t("前置脚本")}><Editor label={t("前置脚本")} value={draft.pre_request_script ?? ""} onChange={pre_request_script => update({ pre_request_script })} dark={state.dark} language="javascript" height="180px"/></Field><Field label={t("后置脚本")}><Editor label={t("后置脚本")} value={draft.post_response_script ?? ""} onChange={post_response_script => update({ post_response_script })} dark={state.dark} language="javascript" height="180px"/></Field></Tabs.Content>
      </Tabs.Root>
      {error && <Callout.Root color="red" role="alert"><Callout.Text>{translateCopy(error)}</Callout.Text></Callout.Root>}
      <Flex justify="end" gap="3"><Dialog.Close><Button variant="soft" color="gray">{t("取消")}</Button></Dialog.Close><Button disabled={!draft.name.trim()} onClick={apply}>{t("应用")}</Button></Flex>
    </Flex>
  </Dialog.Content></Dialog.Root>;
}
