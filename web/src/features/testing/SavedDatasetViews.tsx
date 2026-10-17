import { useState } from "react";
import { Button, Flex, Text, TextArea, TextField } from "@radix-ui/themes";
import { t } from "../../shared/i18n";
import { id } from "../../shared/model";
import { Field } from "../../shared/ui";
import type { SavedDatasetView } from "../../shared/types";

export default function SavedDatasetViews({ views, onChange, disabled }: {
  views: SavedDatasetView[];
  onChange: (views: SavedDatasetView[]) => void;
  disabled: boolean;
}) {
  const [selectedId, setSelectedId] = useState("");
  const selected = views.find(view => view.id === selectedId) ?? views[0];
  const update = (patch: Partial<SavedDatasetView>) => {
    if (selected) onChange(views.map(view => view.id === selected.id ? { ...view, ...patch } : view));
  };
  return <Flex direction="column" gap="2">
    <Text weight="medium" size="2">{t("数据集保存视图")}</Text>
    <Text size="1" color="gray">{t("视图 SQL 随工作区保存；私密数据集的默认备份会移除视图 SQL。")}</Text>
    <Flex gap="2" wrap="wrap">
      {views.map(view => <Button key={view.id} type="button" variant={selected?.id === view.id ? "solid" : "soft"} disabled={disabled} onClick={() => setSelectedId(view.id)}>{view.name || t("未命名视图")}</Button>)}
      <Button type="button" variant="soft" disabled={disabled || views.length >= 20} onClick={() => {
        const view = { id: id(), name: "", sql: "" };
        onChange([...views, view]);
        setSelectedId(view.id);
      }}>{t("添加视图")}</Button>
    </Flex>
    {selected && <>
      <Field label={t("视图名称")}><TextField.Root value={selected.name} maxLength={256} disabled={disabled} onChange={event => update({ name: event.target.value })}/></Field>
      <Field label={t("视图 SQL")} hint={selected.sql === undefined ? t("此视图的 SQL 未包含在备份中，执行前需重新填写。") : t("只允许一条只读 SELECT 或 CTE；使用 ? 绑定参数。")}>
        <TextArea aria-label={t("视图 SQL")} value={selected.sql ?? ""} rows={5} disabled={disabled} onChange={event => update({ sql: event.target.value })}/>
      </Field>
      <Button type="button" color="red" variant="ghost" disabled={disabled} onClick={() => { onChange(views.filter(view => view.id !== selected.id)); setSelectedId(""); }}>{t("删除视图")}</Button>
    </>}
  </Flex>;
}
