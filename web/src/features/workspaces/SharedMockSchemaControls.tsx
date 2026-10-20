import { Checkbox, Text } from "@radix-ui/themes";
import { t, useLanguage } from "../../shared/i18n";
import type { Example } from "../../shared/types";
import { Choice } from "../../shared/ui";
import MockSchemaDialog from "../requests/MockSchemaDialog";

export default function SharedMockSchemaControls({ example, dark, onChange }: { example: Example; dark: boolean; onChange: (patch: Partial<Example>) => void }) {
  useLanguage();
  const source = example.mock_data ? "data" : example.mock_schema_link ? "linked" : example.mock_schema ? "inline" : "static";
  const options = [
    { value: "static", label: t("已保存的静态正文") },
    { value: "inline", label: t("内联 JSON Schema") },
    ...(example.mock_schema_link ? [{ value: "linked", label: t("已保存的规范关联") }] : []),
    ...(example.mock_data ? [{ value: "data", label: t("已保存的数据绑定") }] : []),
  ];
  return <>
    <Choice label={t("Mock 正文来源")} value={source} options={options} onChange={(value) => {
      if (value === "static") onChange({ mock_schema: undefined, mock_schema_link: undefined, mock_data: undefined, mock_name_inference: undefined });
      if (value === "inline") onChange({ mock_schema: example.mock_schema || '{"type":"object","properties":{}}', mock_schema_link: undefined, mock_data: undefined, mock_name_inference: undefined });
    }} />
    {!!example.mock_schema && <MockSchemaDialog schema={example.mock_schema} dark={dark} onCommit={(mock_schema) => onChange({ mock_schema })} />}
    {(!!example.mock_schema || !!example.mock_schema_link) && <Text as="label" size="2"><Checkbox checked={!!example.mock_name_inference} onCheckedChange={(checked) => onChange({ mock_name_inference: checked === true })} /> {t("根据字段名生成 Mock 值")}</Text>}
  </>;
}
