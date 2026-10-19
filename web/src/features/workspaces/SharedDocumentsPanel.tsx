import { useEffect, useState } from "react";
import { AlertDialog, Button, Card, Checkbox, Flex, Heading, Tabs, Text, TextField } from "@radix-ui/themes";
import { t, useLanguage } from "../../shared/i18n";
import { id } from "../../shared/model";
import type { MarkdownDocument, WorkspaceData } from "../../shared/types";
import { Choice, Editor, Field } from "../../shared/ui";
import MarkdownPreview from "../documents/MarkdownPreview";

export default function SharedDocumentsPanel({ data, canEdit, dark, dirty, saving, canSave, conflictRevision, onChange, onSave, onOpenRequest }: {
  data: WorkspaceData;
  canEdit: boolean;
  dark: boolean;
  dirty: boolean;
  saving: boolean;
  canSave: boolean;
  conflictRevision: number | null;
  onChange: (documents: MarkdownDocument[]) => void;
  onSave: () => void;
  onOpenRequest: (id: string) => void;
}) {
  useLanguage();
  const documents = data.documents ?? [];
  const [selectedId, setSelectedId] = useState("");
  const [mode, setMode] = useState("preview");
  const [deleting, setDeleting] = useState<MarkdownDocument | null>(null);
  useEffect(() => { setDeleting(null); }, [conflictRevision]);
  const selected = documents.find((document) => document.id === selectedId) ?? documents[0];
  const requestIds = new Set(data.collections.flatMap((collection) => collection.requests.map((request) => request.id)));
  function update(patch: Partial<MarkdownDocument>) {
    if (!canEdit || !selected) return;
    onChange(documents.map((document) => document.id === selected.id ? { ...document, ...patch } : document));
  }
  function create() {
    if (!canEdit || documents.length >= 256) return;
    const next: MarkdownDocument = { id: id(), name: t("新建文档"), source: "", secret: true };
    onChange([...documents, next]);
    setSelectedId(next.id);
    setMode("edit");
  }
  return <section className="shared-documents" aria-label={t("工作区文档")}>
    <Flex justify="between" align="center" gap="2" wrap="wrap"><Heading size="3">{t("工作区文档")}</Heading>{canEdit && <Button variant="outline" disabled={documents.length >= 256} onClick={create}>{t("新建文档")}</Button>}</Flex>
    {documents.length === 0 ? <Text size="2" color="gray">{t("还没有 Markdown 文档。")}</Text> : <>
      <Choice label={t("文档")} value={selected?.id ?? ""} onChange={(value) => { setSelectedId(value); setMode("preview"); }} options={documents.map((document) => ({ value: document.id, label: document.name }))} />
      {selected && <Card className="shared-document-card">
        <Flex justify="between" align="center" gap="2" wrap="wrap"><Heading size="3">{selected.name}</Heading>{canEdit && <Button variant="outline" color="red" onClick={() => setDeleting(selected)}>{t("删除文档")}</Button>}</Flex>
        {canEdit && <Flex direction="column" gap="3">
          <Field label={t("文档标题")}><TextField.Root value={selected.name} maxLength={128} onChange={(event) => update({ name: event.target.value })} /></Field>
          <Choice label={t("所在目录")} value={selected.collection_id ?? "none"} onChange={(value) => update({ collection_id: value === "none" ? undefined : value })} options={[{ value: "none", label: t("工作区根目录") }, ...data.collections.map((collection) => ({ value: collection.id, label: collection.name }))]} />
          <label className="checkbox-label"><Checkbox checked={selected.secret ?? true} onCheckedChange={(value) => update({ secret: value === true })} />{t("私密文档")}</label>
        </Flex>}
        {canEdit ? <Tabs.Root value={mode} onValueChange={setMode}><Tabs.List><Tabs.Trigger value="edit">{t("编辑")}</Tabs.Trigger><Tabs.Trigger value="preview">{t("预览")}</Tabs.Trigger></Tabs.List><Tabs.Content value="edit"><Editor label={t("Markdown 正文")} value={selected.source} onChange={(source) => update({ source })} dark={dark} language="markdown" height="360px" /></Tabs.Content><Tabs.Content value="preview"><MarkdownPreview nested source={selected.source} requestIds={requestIds} onOpenRequest={onOpenRequest} /></Tabs.Content></Tabs.Root> : selected.secret ? <Text size="2" color="gray">{t("此文档正文未向查看者开放。")}</Text> : <MarkdownPreview nested source={selected.source} requestIds={requestIds} onOpenRequest={onOpenRequest} />}
      </Card>}
    </>}
    {canEdit && <Button loading={saving} disabled={!dirty || !canSave} onClick={onSave}>{t("保存共享工作区")}</Button>}
    <AlertDialog.Root open={!!deleting && conflictRevision === null} onOpenChange={(open) => { if (!open) setDeleting(null); }}><AlertDialog.Content><AlertDialog.Title>{t("删除文档")}</AlertDialog.Title><AlertDialog.Description>{t("删除「{{value0}}」？保存工作区后生效。", { value0: deleting?.name ?? "" })}</AlertDialog.Description><Flex justify="end" gap="3" mt="4"><AlertDialog.Cancel><Button variant="soft" color="gray">{t("取消")}</Button></AlertDialog.Cancel><Button color="red" onClick={() => { if (deleting) onChange(documents.filter((document) => document.id !== deleting.id)); setDeleting(null); setSelectedId(""); }}>{t("确认删除")}</Button></Flex></AlertDialog.Content></AlertDialog.Root>
  </section>;
}
