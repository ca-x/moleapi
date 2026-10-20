import { useEffect, useRef, useState } from "react";
import { AlertDialog, Button, Callout, Checkbox, Flex, Heading, Text, TextField } from "@radix-ui/themes";
import { api, pickFile } from "../../shared/api";
import { t, useLanguage } from "../../shared/i18n";
import { Choice, Editor, Field } from "../../shared/ui";
import type { ImportResult, Specification, WorkspaceData } from "../../shared/types";
import { specificationReferences } from "./specificationReferences";
type Edit = { id?: string; name: string; source: string; includeRequests: boolean; kind: string };
export default function SharedSpecificationsPanel({ data, scope, base, revision, dark, canEdit, dirty, saving, canSave, isCurrent, onChange, onSave }: {
  data: WorkspaceData; scope: string; base: string; revision: number; dark: boolean; canEdit: boolean; dirty: boolean; saving: boolean; canSave: boolean;
  isCurrent: () => boolean; onChange: (data: WorkspaceData) => void; onSave: () => void;
}) {
  useLanguage();
  const [selectedId, setSelectedId] = useState(""), [edit, setEdit] = useState<Edit | null>(null), [pending, setPending] = useState(false), [problem, setProblem] = useState(false), [warnings, setWarnings] = useState<string[]>([]);
  const [deleting, setDeleting] = useState<{ spec: Specification; data: WorkspaceData } | null>(null);
  const specs = data.specifications ?? [], selected = specs.find((spec) => spec.id === selectedId) ?? specs[0];
  const validEdit = !!edit && !!edit.name.trim() && new TextEncoder().encode(edit.name.trim()).length <= 256 && !/[\u0000-\u001f\u007f]/.test(edit.name) && new TextEncoder().encode(edit.source).length <= 5 * 1024 * 1024 && (edit.kind !== "openapi" || !!edit.source.trim());
  const identity = JSON.stringify([scope, selected?.id, edit]);
  const live = useRef(true), sequence = useRef(0), latest = useRef({ identity, data, dirty, saving, isCurrent }); latest.current = { identity, data, dirty, saving, isCurrent };
  useEffect(() => { live.current = true; return () => { live.current = false; sequence.current++; }; }, []);
  const current = (origin: string, ticket: number) => live.current && latest.current.identity === origin && sequence.current === ticket && latest.current.isCurrent();
  async function read() {
    if (!edit || pending) return;
    const origin = identity, ticket = ++sequence.current;
    setPending(true); setProblem(false);
    try { const source = await pickFile({ extensions: ["json", "yaml", "yml"], maximum: 5 * 1024 * 1024, label: "OpenAPI" }); if (source !== null && current(origin, ticket)) setEdit((now) => now ? { ...now, source } : null); }
    catch { if (current(origin, ticket)) setProblem(true); }
    finally { if (live.current && sequence.current === ticket) setPending(false); }
  }
  async function apply() {
    if (!edit || !validEdit || pending || saving || !canEdit) return;
    const origin = identity, ticket = ++sequence.current, snapshot = edit;
    setPending(true); setProblem(false);
    try {
      if (snapshot.kind !== "openapi") {
        if (current(origin, ticket)) onChange({ ...data, specifications: specs.map((spec) => spec.id === snapshot.id ? { ...spec, name: snapshot.name.trim() } : spec) });
      } else {
        if (dirty || !isCurrent()) throw new Error("save canonical workspace first");
        const result = await api<ImportResult>(`${base}/specifications/preview`, "POST", { expected_revision: revision, name: snapshot.name.trim(), source: snapshot.source, ...(snapshot.id ? { specification_id: snapshot.id } : {}), include_requests: !snapshot.id && snapshot.includeRequests });
        if (!current(origin, ticket) || latest.current.dirty || latest.current.saving) return;
        onChange(result.data); setWarnings(result.warnings);
        if (!snapshot.id) setSelectedId(result.data.specifications?.at(-1)?.id ?? "");
      }
      if (live.current) setEdit(null);
    } catch { if (current(origin, ticket)) setProblem(true); }
    finally { if (live.current && sequence.current === ticket) setPending(false); }
  }
  const references = deleting ? specificationReferences(data, deleting.spec.id) : [];
  return <section aria-label={t("API 规范")}><Flex direction="column" gap="3"><Heading size="3">{t("API 规范")}</Heading>
    <Text size="1" color="gray">{t("保留原始规范，与导入的可执行请求关联。")}</Text>
    {canEdit && <Button variant="outline" disabled={pending || saving || !!edit || dirty} onClick={() => { setEdit({ name: "", source: "", includeRequests: true, kind: "openapi" }); setProblem(false); setWarnings([]); }}>{t("导入 OpenAPI 规范")}</Button>}
    {specs.length > 0 && <Choice label={t("规范列表")} value={selected?.id ?? ""} disabled={pending || !!edit} options={specs.map((spec) => ({ value: spec.id, label: `${spec.name} · ${spec.kind} ${spec.dialect}` }))} onChange={(id) => { setSelectedId(id); setProblem(false); setWarnings([]); setDeleting(null); }} />}
    {selected && !edit && <><Text weight="medium">{selected.name}</Text><Editor label={t("规范源文档")} value={selected.source} readOnly dark={dark} height="300px" />{canEdit && <Flex gap="2" wrap="wrap"><Button variant="outline" disabled={saving || (selected.kind === "openapi" && dirty)} onClick={() => { setEdit({ id: selected.id, name: selected.name, source: selected.source, includeRequests: false, kind: selected.kind }); setProblem(false); }}>{t("编辑规范")}</Button><Button variant="outline" color="red" disabled={saving} onClick={() => setDeleting({ spec: selected, data })}>{t("删除规范")}</Button></Flex>}</>}
    {edit && <Flex direction="column" gap="3"><Field label={t("规范名称")}><TextField.Root value={edit.name} disabled={pending || saving} maxLength={256} onChange={(event) => setEdit({ ...edit, name: event.target.value })} /></Field>
      {edit.kind === "openapi" && <><Button variant="soft" disabled={pending || saving} onClick={() => void read()}>{t("选择 OpenAPI 文件")}</Button>{!edit.id && <Text as="label" size="2"><Checkbox checked={edit.includeRequests} disabled={pending || saving} onCheckedChange={(value) => setEdit({ ...edit, includeRequests: value === true })} /> {t("同时导入请求与环境")}</Text>}<Text size="1" color="gray">{edit.id ? t("源文档修改保留现有请求；失效的 Mock 关联会阻止应用。") : t("追加导入的请求与环境；当前环境不会自动切换。")}</Text></>}
      <Editor label={edit.kind === "openapi" ? t("OpenAPI 源文档") : t("规范源文档")} value={edit.source} onChange={(source) => setEdit({ ...edit, source })} readOnly={pending || saving || edit.kind !== "openapi"} dark={dark} height="300px" />
      <Flex gap="2" wrap="wrap"><Button variant="outline" disabled={pending || saving} onClick={() => { setEdit(null); setProblem(false); }}>{t("取消")}</Button><Button loading={pending} disabled={pending || saving || !validEdit || (edit.kind === "openapi" && dirty)} onClick={() => void apply()}>{t("应用规范修改")}</Button></Flex></Flex>}
    {dirty && canEdit && <Text size="1" color="gray">{t("先保存工作区，再校验规范源文档；应用后请再次保存。")}</Text>}
    {problem && <Callout.Root color="red" role="alert"><Callout.Text>{t("规范校验失败，请检查源文档、大小、引用或工作区版本。未应用的编辑已保留。")}</Callout.Text></Callout.Root>}
    {warnings.map((warning, index) => <Text key={index} size="2" color="gray">{warning}</Text>)}
    {canEdit && <Button loading={saving} disabled={!dirty || !canSave || !!edit || pending} onClick={onSave}>{t("保存共享工作区")}</Button>}
  </Flex><AlertDialog.Root open={!!deleting && deleting.data === data && !edit && !saving} onOpenChange={(open) => { if (!open) setDeleting(null); }}><AlertDialog.Content maxWidth="min(560px, calc(100vw - 24px))" style={{ maxHeight: "calc(100dvh - 24px)", overflowY: "auto", overflowWrap: "anywhere" }}><AlertDialog.Title>{t("删除规范")}</AlertDialog.Title><AlertDialog.Description>{t("删除「{{value0}}」，保存后生效。", { value0: deleting?.spec.name ?? "" })}</AlertDialog.Description>{references.length > 0 && <><Text as="p" size="2">{t("规范仍被请求或 Mock 引用，请先更新引用。")}</Text>{references.slice(0, 20).map((reference, index) => <Text as="p" size="2" key={index}>{reference.requestName}{reference.exampleName ? ` / ${reference.exampleName}` : ""}</Text>)}</>}<Flex justify="end" gap="3" mt="4"><AlertDialog.Cancel><Button variant="soft" color="gray">{t("取消")}</Button></AlertDialog.Cancel><Button color="red" disabled={references.length > 0} onClick={() => { if (deleting && deleting.data === data && isCurrent() && !specificationReferences(data, deleting.spec.id).length) { onChange({ ...data, specifications: specs.filter((spec) => spec.id !== deleting.spec.id) }); setSelectedId(""); } setDeleting(null); }}>{t("确认删除")}</Button></Flex></AlertDialog.Content></AlertDialog.Root></section>;
}
