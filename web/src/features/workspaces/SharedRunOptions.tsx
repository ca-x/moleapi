import { Callout, Flex, Text, TextField } from "@radix-ui/themes";
import { t } from "../../shared/i18n";
import { Choice, Field } from "../../shared/ui";
import type { SavedDataset } from "../../shared/types";
import RunnerDataset, { type DatasetPreview } from "../testing/RunnerDataset";
import { resolveSharedRunOptions, type SharedRunConfig } from "./sharedRunOptions";
export default function SharedRunOptions({ value, datasets, scope, workspaceId, disabled, dark, onChange, onBusyChange, previewSource }: {
  value: SharedRunConfig; datasets: SavedDataset[]; scope: string; workspaceId: string; disabled: boolean; dark: boolean;
  onChange: (value: SharedRunConfig) => void; onBusyChange: (busy: boolean) => void;
  previewSource: (source: { format: "csv" | "json"; source: string }) => Promise<DatasetPreview>;
}) {
  const problem = resolveSharedRunOptions(value, datasets).problem;
  const missing = value.selection.startsWith("dataset:") && !datasets.some((item) => `dataset:${item.id}` === value.selection);
  return <section aria-label={t("运行数据设置")}><Flex direction="column" gap="3">
    <Flex gap="3" wrap="wrap">
      <Choice label={t("运行数据集")} value={value.selection} disabled={disabled} options={[
        { value: "none", label: t("不使用数据集") }, { value: "temporary", label: t("临时 CSV/JSON") },
        ...datasets.map((item) => ({ value: `dataset:${item.id}`, label: item.name })),
        ...(missing ? [{ value: value.selection, label: t("数据集不可用") }] : []),
      ]} onChange={(selection) => onChange({ ...value, selection })} />
      <Field label={t("迭代次数")}><TextField.Root type="number" min="1" max="100" disabled={disabled} placeholder={t("自动：数据行数或 1")} value={value.iterations} onChange={(event) => onChange({ ...value, iterations: event.target.value })} /></Field>
    </Flex>
    {value.selection === "temporary" && <><Text size="1" color="gray">{t("临时数据仅用于本次运行，不保存到工作区。")}</Text><RunnerDataset workspaceId={workspaceId} scope={scope} value={value} disabled={disabled} dark={dark} showIterations={false} onChange={(config) => onChange({ ...value, ...config })} onBusyChange={onBusyChange} previewSource={previewSource} /></>}
    {problem && <Callout.Root color="amber" role="alert"><Callout.Text>{problem === "dataset" ? t("所选数据集已删除或缺少数据源，请重新选择。") : t("测试数据无效，请检查格式、迭代次数和大小限制。")}</Callout.Text></Callout.Root>}
  </Flex></section>;
}
