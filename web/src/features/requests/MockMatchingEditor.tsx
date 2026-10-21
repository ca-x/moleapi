import { Button, Checkbox, Text, TextArea, TextField } from "@radix-ui/themes";
import { t, useLanguage } from "../../shared/i18n";
import type { Example } from "../../shared/types";
import { Field, PairEditor } from "../../shared/ui";

export default function MockMatchingEditor({ rule, onChange }: { rule: Example["mock_match"]; onChange: (rule: Example["mock_match"]) => void }) {
  useLanguage();
  if (!rule) return <Button variant="outline" color="gray" onClick={() => onChange({ priority: 0, query: [], headers: [], body: "" })}>{t("添加 Mock 匹配条件")}</Button>;
  return <div>
    <Text size="1" color="gray">{t("匹配条件全部满足时选中此示例；空正文只匹配无正文请求。")}</Text>
    <Field label={t("匹配优先级")}><TextField.Root type="number" min="0" max="100" step="1" value={rule.priority} onChange={(event) => {
      const value = Number(event.target.value);
      if (Number.isFinite(value)) onChange({ ...rule, priority: Math.min(100, Math.max(0, Math.round(value))) });
    }} /></Field>
    <Field label={t("查询参数条件")}><PairEditor rows={rule.query ?? []} onChange={(query) => onChange({ ...rule, query })} keyLabel={t("参数名")} valueLabel={t("参数值")} secrets /></Field>
    <Field label={t("请求头条件")}><PairEditor rows={rule.headers ?? []} onChange={(headers) => onChange({ ...rule, headers })} keyLabel="Header" valueLabel={t("值")} secrets /></Field>
    <Text as="label" size="2"><Checkbox checked={rule.body !== undefined} onCheckedChange={(checked) => onChange({ ...rule, body: checked === true ? "" : undefined })} />{t("匹配正文")}</Text>
    {rule.body !== undefined && <Field label={t("精确匹配正文")}><TextArea value={rule.body} onChange={(event) => onChange({ ...rule, body: event.target.value })} rows={3} /></Field>}
    <Button variant="ghost" color="red" onClick={() => onChange(undefined)}>{t("移除匹配条件")}</Button>
  </div>;
}
