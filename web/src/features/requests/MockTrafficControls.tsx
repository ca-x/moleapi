import { TextField } from "@radix-ui/themes";
import { t, useLanguage } from "../../shared/i18n";
import type { Example } from "../../shared/types";
import { Field } from "../../shared/ui";


export default function MockTrafficControls({ example, onChange }: { example: Example; onChange: (patch: Partial<Example>) => void }) {
  useLanguage();
  const controls = [
    { key: "delay_ms", label: t("Mock 延迟（毫秒）"), hint: t("0 表示立即响应；最多等待 30000 毫秒。"), min: 0, max: 30_000, fallback: 0 },
    { key: "fault_rate_percent", label: t("故障比例（%）"), hint: t("0 表示关闭；命中时返回指定的 5xx 状态码。"), min: 0, max: 100, fallback: 0 },
    { key: "fault_status", label: t("故障状态码（5xx）"), hint: t("仅用于模拟故障；响应正文固定且不包含保存的示例数据。"), min: 500, max: 599, fallback: 503 },
    { key: "rate_limit_per_minute", label: t("每分钟请求上限"), hint: t("0 表示关闭；滑动 60 秒内超过上限立即返回 429，最多 1000 次。"), min: 0, max: 1000, fallback: 0 },
  ] as const;
  return <>{controls.map((control) => <Field key={control.key} label={control.label} hint={control.hint}>
    <TextField.Root type="number" min={control.min} max={control.max} step="1" value={example[control.key] ?? control.fallback} onChange={(event) => {
      const value = Number(event.target.value);
      if (Number.isFinite(value)) onChange({ [control.key]: Math.min(control.max, Math.max(control.min, Math.round(value))) });
    }} />
  </Field>)}</>;
}
