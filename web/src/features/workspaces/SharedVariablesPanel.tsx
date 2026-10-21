import { useEffect, useState } from "react";
import { AlertDialog, Button, Card, Flex, Heading, Tabs, Text, TextField } from "@radix-ui/themes";
import { t, useLanguage } from "../../shared/i18n";
import { id } from "../../shared/model";
import type { Pair, WorkspaceData } from "../../shared/types";
import { Choice, Field, PairEditor } from "../../shared/ui";

export default function SharedVariablesPanel({ data, canEdit, dirty, saving, canSave, conflictRevision, onChange, onSave }: {
  data: WorkspaceData;
  canEdit: boolean;
  dirty: boolean;
  saving: boolean;
  canSave: boolean;
  conflictRevision: number | null;
  onChange: (data: WorkspaceData) => void;
  onSave: () => void;
}) {
  useLanguage();
  const [selectedEnvironmentId, setSelectedEnvironmentId] = useState("");
  const [selectedCollectionId, setSelectedCollectionId] = useState("");
  const [deleting, setDeleting] = useState<{ id: string; name: string } | null>(null);
  useEffect(() => { setDeleting(null); }, [conflictRevision]);
  const environment = data.environments.find((item) => item.id === selectedEnvironmentId)
    ?? data.environments.find((item) => item.id === data.active_environment_id)
    ?? data.environments[0];
  const collection = data.collections.find((item) => item.id === selectedCollectionId) ?? data.collections[0];
  function rows(value: Pair[], change: (next: Pair[]) => void) {
    if (canEdit) return <PairEditor rows={value} secrets onChange={change} keyLabel={t("变量名")} valueLabel={t("共享值")} />;
    return value.length === 0 ? <Text size="2" color="gray">{t("暂无变量")}</Text> : <Flex direction="column" gap="2">{value.map((item) =>
      <Flex key={item.id} gap="2" wrap="wrap"><Text size="2" weight="medium">{item.key}</Text><Text size="2" color="gray">{item.secret ? t("私密值已隐藏") : item.value || t("空值或已隐藏")}</Text>{!item.enabled && <Text size="2" color="gray">{t("已停用")}</Text>}</Flex>,
    )}</Flex>;
  }
  return <section className="shared-variables" aria-label={t("环境变量")}>
    <Flex direction="column" gap="3">
      <Heading size="3">{t("环境变量")}</Heading>
      <Text size="2" color="gray">{canEdit ? t("成员编辑的是工作区共享值；本地覆盖值仍属于各自账号。") : t("这里只显示工作区共享值；私密值已隐藏。")}</Text>
      <Tabs.Root defaultValue="environment">
        <Tabs.List><Tabs.Trigger value="environment">{t("环境")}</Tabs.Trigger><Tabs.Trigger value="project">{t("项目全局")}</Tabs.Trigger><Tabs.Trigger value="collection">{t("集合变量")}</Tabs.Trigger></Tabs.List>
        <Tabs.Content value="environment">
          <Flex direction="column" gap="3" mt="3">
            {canEdit && <Button variant="outline" onClick={() => {
              const next = { id: id(), name: t("新建环境"), variables: [] };
              onChange({ ...data, environments: [...data.environments, next], active_environment_id: next.id });
              setSelectedEnvironmentId(next.id);
            }}>{t("新建环境")}</Button>}
            {environment ? <Card className="environment-card"><Flex direction="column" gap="3">
              <Choice label={t("环境")} value={environment.id} onChange={setSelectedEnvironmentId} options={data.environments.map((item) => ({ value: item.id, label: item.name }))} />
              {canEdit ? <>
                <Field label={t("环境名称")}><TextField.Root value={environment.name} onChange={(event) => onChange({ ...data, environments: data.environments.map((item) => item.id === environment.id ? { ...item, name: event.target.value } : item) })} /></Field>
                <Flex gap="2" wrap="wrap"><Button variant="outline" disabled={data.active_environment_id === environment.id} onClick={() => onChange({ ...data, active_environment_id: environment.id })}>{t("设为活动环境")}</Button><Button variant="outline" color="red" onClick={() => setDeleting({ id: environment.id, name: environment.name })}>{t("删除当前环境")}</Button></Flex>
              </> : <Text size="2">{environment.name}</Text>}
              {rows(environment.variables, (variables) => onChange({ ...data, environments: data.environments.map((item) => item.id === environment.id ? { ...item, variables } : item) }))}
            </Flex></Card> : <Text size="2" color="gray">{t("选择或新建一个环境。")}</Text>}
          </Flex>
        </Tabs.Content>
        <Tabs.Content value="project"><Card className="scope-variables-card" mt="3">{rows(data.global_variables ?? [], (global_variables) => onChange({ ...data, global_variables }))}</Card></Tabs.Content>
        <Tabs.Content value="collection"><Card className="scope-variables-card" mt="3"><Flex direction="column" gap="3">{collection ? <><Choice label={t("变量所属集合")} value={collection.id} onChange={setSelectedCollectionId} options={data.collections.map((item) => ({ value: item.id, label: item.name }))} />{rows(collection.variables ?? [], (variables) => onChange({ ...data, collections: data.collections.map((item) => item.id === collection.id ? { ...item, variables } : item) }))}</> : <Text size="2" color="gray">{t("先创建一个集合。")}</Text>}</Flex></Card></Tabs.Content>
      </Tabs.Root>
      {canEdit && <Button loading={saving} disabled={!dirty || !canSave} onClick={onSave}>{t("保存共享工作区")}</Button>}
    </Flex>
    <AlertDialog.Root open={!!deleting && conflictRevision === null} onOpenChange={(open) => { if (!open) setDeleting(null); }}><AlertDialog.Content><AlertDialog.Title>{t("删除环境")}</AlertDialog.Title><AlertDialog.Description>{t("删除「{{value0}}」？保存工作区后生效。", { value0: deleting?.name ?? "" })}</AlertDialog.Description><Flex justify="end" gap="3" mt="4"><AlertDialog.Cancel><Button variant="soft" color="gray">{t("取消")}</Button></AlertDialog.Cancel><Button color="red" onClick={() => { if (deleting) { const environments = data.environments.filter((item) => item.id !== deleting.id); onChange({ ...data, environments, active_environment_id: data.active_environment_id === deleting.id ? environments[0]?.id ?? null : data.active_environment_id }); } setDeleting(null); setSelectedEnvironmentId(""); }}>{t("确认删除")}</Button></Flex></AlertDialog.Content></AlertDialog.Root>
  </section>;
}
