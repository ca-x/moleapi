import { useMemo } from "react";
import { Button, Checkbox, Text } from "@radix-ui/themes";
import { t, useLanguage } from "../../shared/i18n";
import type { Example, RequestSpec, SavedDataset, Specification } from "../../shared/types";
import { Choice } from "../../shared/ui";
import MockSchemaDialog from "../requests/MockSchemaDialog";
import { openapiMockChoices } from "../requests/openapiMockChoices";

export default function SharedMockSchemaControls({ example, request, specifications, dark, datasets, dataEnabled, onSelectLinked, onChange }: { example: Example; request: RequestSpec; specifications: Specification[]; dark: boolean; datasets: SavedDataset[]; dataEnabled: boolean; onSelectLinked: (pointer: string) => void; onChange: (patch: Partial<Example>) => void }) {
  useLanguage();
  const firstDataset = datasets.find((dataset) => dataset.views?.some((view) => !!view.sql));
  const linkedSpec = (!request.protocol?.kind || request.protocol.kind === "http") ? specifications.find((spec) => spec.id === request.specification_id && spec.kind === "openapi") : undefined;
  const choices = useMemo(() => linkedSpec ? openapiMockChoices(linkedSpec.source, request.method) : [], [linkedSpec?.source, request.method]);
  const validLink = !!linkedSpec && example.mock_schema_link?.specification_id === linkedSpec.id && choices.some((choice) => choice.pointer === example.mock_schema_link?.pointer);
  const source = example.mock_data ? "data" : example.mock_schema_link ? validLink ? example.mock_schema_link.pointer : "linked" : example.mock_schema ? "inline" : "static";
  const options = [
    { value: "static", label: t("已保存的静态正文") },
    { value: "inline", label: t("内联 JSON Schema") },
    ...choices.map((choice) => ({ value: choice.pointer, label: choice.label })),
    ...(example.mock_schema_link && !validLink ? [{ value: "linked", label: t("已保存的规范关联") }] : []),
    ...(example.mock_data || (dataEnabled && firstDataset) ? [{ value: "data", label: t("已保存的数据绑定") }] : []),
  ];
  return <>
    {linkedSpec && <Text size="1" color="gray">{t("来源规范：{{name}}", { name: linkedSpec.name })}</Text>}
    <Choice label={t("Mock 正文来源")} value={source} options={options} onChange={(value) => {
      if (choices.some((choice) => choice.pointer === value)) onSelectLinked(value);
      if (value === "data" && !example.mock_data && dataEnabled && firstDataset) {
        const view = firstDataset.views?.find((item) => !!item.sql);
        if (view) onChange({ mock_data: { dataset_id: firstDataset.id, view_id: view.id, shape: "rows", parameters: [] }, mock_schema: undefined, mock_schema_link: undefined, mock_name_inference: undefined, headers: example.headers.map((header) => header.enabled && header.key.toLowerCase() === "content-type" ? { ...header, value: "application/json" } : header) });
      }
      if (value === "static") onChange({ mock_schema: undefined, mock_schema_link: undefined, mock_data: undefined, mock_name_inference: undefined });
      if (value === "inline") onChange({ mock_schema: example.mock_schema || '{"type":"object","properties":{}}', mock_schema_link: undefined, mock_data: undefined, mock_name_inference: undefined });
    }} />
    {linkedSpec && choices.length === 0 && <Text size="1" color="gray">{t("规范中没有可选的 JSON 响应。")}</Text>}
    {example.mock_schema_link && !validLink && <><Text role="alert" size="1" color="red">{linkedSpec ? t("关联的响应与当前规范不匹配；请重新选择。") : t("关联的 OpenAPI 规范不可用；请选择静态正文或内联 Schema。")}</Text><Button variant="outline" color="gray" onClick={() => onChange({ mock_schema_link: undefined, mock_name_inference: undefined })}>{t("清除 Schema 关联")}</Button></>}
    {dataEnabled && !firstDataset && <Text size="1" color="gray">{t("请先保存带只读视图的数据集。")}</Text>}
    {!!example.mock_schema && <MockSchemaDialog schema={example.mock_schema} dark={dark} onCommit={(mock_schema) => onChange({ mock_schema })} />}
    {(!!example.mock_schema || !!example.mock_schema_link) && <Text as="label" size="2"><Checkbox checked={!!example.mock_name_inference} onCheckedChange={(checked) => onChange({ mock_name_inference: checked === true })} /> {t("根据字段名生成 Mock 值")}</Text>}
  </>;
}
