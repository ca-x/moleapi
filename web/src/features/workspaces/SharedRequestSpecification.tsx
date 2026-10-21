import { useState } from "react";
import { Text } from "@radix-ui/themes";
import { t, useLanguage } from "../../shared/i18n";
import { Choice } from "../../shared/ui";
import type { RequestSpec, Specification } from "../../shared/types";
export default function SharedRequestSpecification({ request, specifications, onChange }: { request: RequestSpec; specifications: Specification[]; onChange: (patch: Partial<RequestSpec>) => void }) {
  useLanguage();
  const [blocked, setBlocked] = useState(false);
  const available = specifications.filter((spec) => spec.kind === "openapi");
  const missing = !!request.specification_id && !available.some((spec) => spec.id === request.specification_id);
  const links = request.examples.filter((example) => !!example.mock_schema_link);
  return <><Choice label={t("关联 API 规范")} value={request.specification_id ? `spec:${request.specification_id}` : "none"} options={[
    { value: "none", label: t("不关联规范") }, ...available.map((spec) => ({ value: `spec:${spec.id}`, label: spec.name })),
    ...(missing ? [{ value: `spec:${request.specification_id}`, label: t("已保存的规范关联不可用") }] : []),
  ]} onChange={(value) => {
    const id = value.startsWith("spec:") ? value.slice(5) : null;
    if (id === (request.specification_id ?? null)) return;
    if (links.length) { setBlocked(true); return; }
    if (id !== null && !available.some((spec) => spec.id === id)) return;
    setBlocked(false); onChange({ specification_id: id, operation_id: null });
  }} />{blocked && <Text role="alert" size="2" style={{ overflowWrap: "anywhere" }}>{t("请先清除响应示例的规范关联，再切换请求规范。")} {links.map((example) => example.name).join(" · ")}</Text>}</>;
}
