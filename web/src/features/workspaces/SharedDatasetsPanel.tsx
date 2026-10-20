import { useEffect, useRef, useState } from "react";
import { AlertDialog, Button, Callout, Card, Flex, Heading, Text } from "@radix-ui/themes";
import { api } from "../../shared/api";
import { t, useLanguage } from "../../shared/i18n";
import type { SavedDataset, WorkspaceData } from "../../shared/types";
import DatasetFields from "../testing/DatasetFields";
import RunnerDataset, { type DatasetPreview } from "../testing/RunnerDataset";
import { datasetEdit, datasetFromEdit, validDatasetEdit, type DatasetEdit } from "../testing/datasetEdit";
import { datasetBindings } from "./datasetReferences";

export default function SharedDatasetsPanel({ data, scope, base, revision, dark, dirty, saving, canSave, isCurrent, onChange, onSave }: { data: WorkspaceData; scope: string; base: string; revision: number; dark: boolean; dirty: boolean; saving: boolean; canSave: boolean; isCurrent: () => boolean; onChange: (datasets: SavedDataset[]) => void; onSave: () => void }) {
  useLanguage();
  const [edit, setEdit] = useState<{ value: DatasetEdit; original: string | null } | null>(null);
  const [pending, setPending] = useState(false), [sourceBusy, setSourceBusy] = useState(false), [error, setError] = useState(false);
  const [deleting, setDeleting] = useState<{ id: string; name: string; data: WorkspaceData } | null>(null);
  const mounted = useRef(true), sequence = useRef(0);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; sequence.current++; }; }, []);
  const identity = JSON.stringify([scope, edit]);
  const latest = useRef({ identity, data }); latest.current = { identity, data };
  const current = (origin: string, ticket: number) => mounted.current && latest.current.identity === origin && sequence.current === ticket && isCurrent();
  const datasets = data.datasets ?? [];
  const requests = data.collections.flatMap((collection) => collection.requests.filter((request) => request.protocol?.kind === "data" && request.protocol.read_only && request.protocol.sql.trim()).map((request) => ({ value: request.id, label: `${collection.name} / ${request.name}` })));
  const busy = saving || pending || sourceBusy;
  const affected = edit?.value.id ? datasetBindings(data, edit.value.id, edit.value.views) : [];
  const deleteBindings = deleting ? datasetBindings(data, deleting.id) : [];
  const preview = (source: { format: "csv" | "json"; source: string }) => api<DatasetPreview>(`${base}/dataset/preview`, "POST", { expected_revision: revision, dataset: source });
  async function apply() {
    if (!edit || busy || affected.length || !validDatasetEdit(edit.value, datasets, requests)) return;
    const origin = identity, ticket = ++sequence.current, snapshot = structuredClone(edit);
    setPending(true); setError(false);
    try {
      if (snapshot.value.mode === "literal" && snapshot.value.source.trim()) await preview({ format: snapshot.value.format, source: snapshot.value.source });
      if (!current(origin, ticket)) return;
      const now = latest.current.data, existing = now.datasets?.find((dataset) => dataset.id === snapshot.value.id);
      if (snapshot.original !== null && JSON.stringify(existing) !== snapshot.original) throw new Error("dataset changed");
      const value = datasetFromEdit(snapshot.value);
      if (datasetBindings(now, value.id, value.views ?? []).length) throw new Error("referenced view");
      onChange([...(now.datasets ?? []).filter((dataset) => dataset.id !== value.id), value]); setEdit(null);
    } catch { if (current(origin, ticket)) setError(true); }
    finally { if (mounted.current && sequence.current === ticket) setPending(false); }
  }
  const bindings = affected.length ? affected : deleting ? deleteBindings : [];
  return <section aria-label={t("工作区数据集")}><Flex direction="column" gap="3">
    <Heading size="3">{t("工作区数据集")}</Heading><Text size="1" color="gray">{t("数据集随工作区保存和同步。默认备份隐藏私密数据源；包含私密值的备份保留原文。")}</Text>
    {datasets.map((dataset) => <Card key={dataset.id}><Flex gap="2" align="center" wrap="wrap"><Text weight="medium">{dataset.name}</Text>{dataset.query && <Text size="1" color="gray">{t("Data 查询（每次运行刷新）")}</Text>}{!dataset.source && !dataset.query && <Text size="1" color="gray">{t("数据源缺失，请重新导入。")}</Text>}<Button variant="outline" disabled={busy} onClick={() => { setEdit({ value: datasetEdit(structuredClone(dataset)), original: JSON.stringify(dataset) }); setError(false); }}>{t("编辑数据集")}</Button><Button variant="outline" color="red" disabled={busy || !!edit} onClick={() => setDeleting({ id: dataset.id, name: dataset.name, data })}>{t("删除数据集")}</Button></Flex></Card>)}
    <Button variant="outline" disabled={busy || datasets.length >= 20} onClick={() => { setEdit({ value: datasetEdit(), original: null }); setError(false); }}>{t("新建数据集")}</Button>
    {edit && <Card><Flex direction="column" gap="3"><DatasetFields edit={edit.value} requests={requests} scope={scope} disabled={busy} onChange={(value) => setEdit((now) => now ? { ...now, value } : null)} literalSource={<RunnerDataset workspaceId="" scope={identity} value={{ format: edit.value.format, source: edit.value.source, iterations: "" }} disabled={saving || pending} dark={dark} showIterations={false} previewSource={preview} onBusyChange={setSourceBusy} onChange={(value) => setEdit((now) => now ? { ...now, value: { ...now.value, format: value.format, source: value.source } } : null)} />} /><Text size="1" color="gray">{t("应用数据集修改后，再保存共享工作区。")}</Text><Flex gap="2" wrap="wrap"><Button variant="outline" disabled={pending || sourceBusy} onClick={() => { setEdit(null); setError(false); }}>{t("取消")}</Button><Button loading={pending} disabled={busy || affected.length > 0 || !validDatasetEdit(edit.value, datasets, requests)} onClick={() => void apply()}>{t("应用数据集修改")}</Button></Flex></Flex></Card>}
    {affected.length > 0 && <Callout.Root color="gray"><Callout.Text>{t("Mock 绑定仍引用此数据集或视图。请先更新绑定。")}</Callout.Text>{bindings.slice(0,20).map((binding) => <Text as="p" size="2" key={`${binding.requestId}/${binding.exampleId}`} style={{ overflowWrap: "anywhere" }}>{binding.requestName} / {binding.exampleName} · <code>{binding.viewId}</code></Text>)}</Callout.Root>}
    {error && <Callout.Root color="red" role="alert"><Callout.Text>{t("数据集保存失败，请检查数据、名称、大小或工作区版本冲突。未保存的编辑已保留。")}</Callout.Text></Callout.Root>}
    <Button loading={saving} disabled={!dirty || !canSave || !!edit || pending || sourceBusy} onClick={onSave}>{t("保存共享工作区")}</Button>
  </Flex><AlertDialog.Root open={!!deleting && deleting.data === data && !busy} onOpenChange={(open) => { if (!open) setDeleting(null); }}><AlertDialog.Content maxWidth="min(600px, calc(100vw - 24px))" style={{ maxHeight: "calc(100dvh - 24px)", overflowY: "auto" }}><AlertDialog.Title>{t("删除数据集")}</AlertDialog.Title><AlertDialog.Description>{t("删除「{{value0}}」，保存后生效。", { value0: deleting?.name ?? "" })}</AlertDialog.Description>{deleteBindings.length > 0 && <Callout.Root color="gray" style={{ overflowWrap: "anywhere" }}><Callout.Text>{t("Mock 绑定仍引用此数据集或视图。请先更新绑定。")}</Callout.Text>{deleteBindings.slice(0,20).map((binding) => <Text as="p" size="2" key={`${binding.requestId}/${binding.exampleId}`}>{binding.requestName} / {binding.exampleName} · <code>{binding.viewId}</code></Text>)}</Callout.Root>}<Flex justify="end" gap="3" mt="4"><AlertDialog.Cancel><Button variant="soft" color="gray">{t("取消")}</Button></AlertDialog.Cancel><Button color="red" disabled={deleteBindings.length > 0} onClick={() => { if (deleting && deleting.data === data && !busy && isCurrent() && !datasetBindings(data, deleting.id).length) onChange(datasets.filter((dataset) => dataset.id !== deleting.id)); setDeleting(null); }}>{t("确认删除")}</Button></Flex></AlertDialog.Content></AlertDialog.Root></section>;
}
