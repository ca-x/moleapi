import { useState } from "react";
import { AlertDialog, Button, Callout, Card, Flex, Heading, Text } from "@radix-ui/themes";
import { t, useLanguage } from "../../shared/i18n";
import { id } from "../../shared/model";
import { Choice } from "../../shared/ui";
import type { Scenario, WorkspaceData } from "../../shared/types";
import { collectionDescendants } from "../collections/tree";
import ScenarioEditor, { scenarioDraftValid } from "../testing/ScenarioEditor";

type RunResult = { passed: number; failed: number; cancelled: boolean };
export default function SharedScenariosPanel({ data, collectionId, dirty, saving, canSave, running, runOptionsValid = true, result, onSelectCollection, onChange, onSave, onRun }: {
  data: WorkspaceData; collectionId: string; dirty: boolean; saving: boolean; canSave: boolean; running: boolean; runOptionsValid?: boolean; result: RunResult | null;
  onSelectCollection: (id: string) => void; onChange: (scenarios: Scenario[]) => void; onSave: () => void; onRun: (id: string) => void;
}) {
  useLanguage();
  const [edit, setEdit] = useState<{ value: Scenario; original: string | null } | null>(null);
  const [deleting, setDeleting] = useState<{ id: string; name: string; data: WorkspaceData } | null>(null);
  const [error, setError] = useState(false);
  const scenarios = (data.scenarios ?? []).filter((scenario) => scenario.collection_id === collectionId);
  const subtree = collectionDescendants(data.collections, collectionId);
  const requests = data.collections.filter((collection) => subtree.has(collection.id)).flatMap((collection) => collection.requests.map((request) => ({ value: request.id, label: `${collection.name} / ${request.name}` })));
  const busy = saving || running;
  function apply() {
    if (!edit || busy) return;
    const value = { ...edit.value, id: edit.value.id || id(), name: edit.value.name.trim() };
    const current = data.scenarios?.find((scenario) => scenario.id === value.id);
    if (!scenarioDraftValid(value) || value.collection_id !== collectionId || value.steps.some((step) => !requests.some((request) => request.value === step.request_id)) || (edit.original !== null && JSON.stringify(current) !== edit.original)) { setError(true); return; }
    onChange([...(data.scenarios ?? []).filter((scenario) => scenario.id !== value.id), value]);
    setEdit(null); setError(false);
  }
  return <section aria-label={t("工作区测试场景")}><Flex direction="column" gap="3">
    <Heading size="3">{t("工作区测试场景")}</Heading>
    {data.collections.length > 0 && <Choice label={t("集合")} value={collectionId} disabled={busy} options={data.collections.map((collection) => ({ value: collection.id, label: collection.name }))} onChange={onSelectCollection} />}
    <Text size="1" color="gray">{t("场景保存请求引用、顺序和分组，随工作区同步。步骤使用原接口定义及当前环境，允许重复执行同一请求。")}</Text>
    {scenarios.map((scenario) => <Card key={scenario.id}><Flex gap="2" wrap="wrap" align="center"><Text weight="medium">{scenario.name}</Text><Button variant="outline" disabled={busy} onClick={() => { setEdit({ value: structuredClone(scenario), original: JSON.stringify(scenario) }); setError(false); }}>{t("编辑场景")}</Button><Button variant="outline" color="red" disabled={busy || !!edit} onClick={() => setDeleting({ id: scenario.id, name: scenario.name, data })}>{t("删除场景")}</Button><Button disabled={busy || dirty || !!edit || !runOptionsValid} onClick={() => onRun(scenario.id)}>{t("运行已保存场景")}</Button></Flex></Card>)}
    <Button variant="outline" disabled={busy || !collectionId} onClick={() => { setEdit({ value: { id: "", name: "", description: "", collection_id: collectionId, steps: [] }, original: null }); setError(false); }}>{t("新建场景")}</Button>
    {edit && <Card><Flex direction="column" gap="3"><ScenarioEditor key={edit.value.id || "new"} scenario={edit.value} requests={requests} disabled={busy} onChange={(value) => setEdit((current) => current ? { ...current, value } : null)} /><Text size="1" color="gray">{t("应用场景修改后，再保存共享工作区。")}</Text><Flex gap="2" wrap="wrap"><Button variant="outline" disabled={busy} onClick={() => { setEdit(null); setError(false); }}>{t("取消")}</Button><Button disabled={busy || !scenarioDraftValid(edit.value)} onClick={apply}>{t("应用场景修改")}</Button></Flex></Flex></Card>}
    {error && <Callout.Root color="red" role="alert"><Callout.Text>{t("场景保存失败，请检查名称、请求引用、步骤数量或工作区版本冲突。未保存的编辑已保留。")}</Callout.Text></Callout.Root>}
    {(dirty || !!edit) && <Text size="1" color="gray">{t("先应用场景修改并保存工作区，再运行场景。")}</Text>}
    {result && <Text role="status" size="2">{t("通过")}: {result.passed} · {t("失败")}: {result.failed}{result.cancelled && <> · {t("已取消")}</>}</Text>}
    <Button loading={saving} disabled={!dirty || !canSave || !!edit} onClick={onSave}>{t("保存共享工作区")}</Button>
  </Flex><AlertDialog.Root open={!!deleting && deleting.data === data && !busy} onOpenChange={(open) => { if (!open) setDeleting(null); }}><AlertDialog.Content maxWidth="min(560px, calc(100vw - 24px))"><AlertDialog.Title>{t("删除场景")}</AlertDialog.Title><AlertDialog.Description>{t("删除「{{value0}}」，保存后生效。", { value0: deleting?.name ?? "" })}</AlertDialog.Description><Flex justify="end" gap="3" mt="4"><AlertDialog.Cancel><Button variant="soft" color="gray">{t("取消")}</Button></AlertDialog.Cancel><Button color="red" onClick={() => { if (deleting && deleting.data === data && !busy) onChange((data.scenarios ?? []).filter((scenario) => scenario.id !== deleting.id)); setDeleting(null); }}>{t("确认删除")}</Button></Flex></AlertDialog.Content></AlertDialog.Root></section>;
}
