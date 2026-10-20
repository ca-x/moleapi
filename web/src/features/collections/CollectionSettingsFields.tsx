import { Checkbox, Flex, Tabs, Text, TextField } from "@radix-ui/themes";
import { Choice, Editor, Field, PairEditor } from "../../shared/ui";
import { t, useLanguage } from "../../shared/i18n";
import type { Auth, Collection } from "../../shared/types";
import RequestAuthEditor from "../authentication/RequestAuthEditor";
import { collectionDescendants } from "./tree";

export type CollectionSettingsTarget = { kind: "edit"; id: string } | { kind: "create"; parent_id: string | null };
const inherited = (): Auth => ({ kind: "inherit", token: "", username: "", password: "" });

export default function CollectionSettingsFields({ collection, collections, dark, tokenManagement = true, onChange }: { collection: Collection; collections: Collection[]; dark: boolean; tokenManagement?: boolean; onChange: (patch: Partial<Collection>) => void }) {
  useLanguage();
  const excluded = collectionDescendants(collections, collection.id);
  return <>
      <Field label={t("名称")}><TextField.Root value={collection.name} maxLength={256} onChange={e => onChange({ name: e.target.value })}/></Field>
      <Choice label={t("父目录")} value={collection.parent_id ? `collection:${collection.parent_id}` : "root"} options={[{ value: "root", label: t("工作区根目录") }, ...collections.filter(c => !excluded.has(c.id)).map(c => ({ value: `collection:${c.id}`, label: c.name }))]} onChange={parent => onChange({ parent_id: parent === "root" ? null : parent.slice("collection:".length) })}/>
      <Tabs.Root defaultValue="auth"><Tabs.List><Tabs.Trigger value="auth">{t("鉴权")}</Tabs.Trigger><Tabs.Trigger value="variables">{t("变量")}</Tabs.Trigger><Tabs.Trigger value="scripts">{t("脚本")}</Tabs.Trigger></Tabs.List>
        <Tabs.Content value="auth"><RequestAuthEditor auth={collection.auth ?? inherited()} change={auth => onChange({ auth })} dark={dark} collectionId={collection.id} tokenManagement={tokenManagement}/></Tabs.Content>
        <Tabs.Content value="variables"><Text as="label" size="2"><Flex gap="2" align="center"><Checkbox checked={collection.variables_enabled!==false} onCheckedChange={value=>onChange({variables_enabled:value===true})}/>{t("执行目录变量")}</Flex></Text>{collection.variables_enabled===false && <Text as="p" size="1" color="gray">{t("目录变量仅保留源定义；启用后由 MoleAPI 在请求中解析。")}</Text>}<PairEditor rows={collection.variables ?? []} onChange={variables => onChange({ variables })} secrets keyLabel={t("变量名")} valueLabel={t("共享值")}/><Text size="1" color="gray">{t("同名变量按父级到子级覆盖，环境变量优先于集合变量。")}</Text></Tabs.Content>
        <Tabs.Content value="scripts"><Field label={t("前置脚本")}><Editor label={t("前置脚本")} value={collection.pre_request_script ?? ""} onChange={pre_request_script => onChange({ pre_request_script })} dark={dark} language="javascript" height="180px"/></Field><Field label={t("后置脚本")}><Editor label={t("后置脚本")} value={collection.post_response_script ?? ""} onChange={post_response_script => onChange({ post_response_script })} dark={dark} language="javascript" height="180px"/></Field></Tabs.Content>
      </Tabs.Root>
  </>;
}
