import { Text } from "@radix-ui/themes";
import { t, useLanguage } from "../../shared/i18n";
import { Editor, Field } from "../../shared/ui";

export default function MockScriptEditor({ script, enabled, dark, onChange }: { script: string; enabled: boolean; dark: boolean; onChange: (script: string) => void }) {
  useLanguage();
  return enabled ? <Field label={t("Mock 实现脚本")} hint={t("使用 pm.mock 设置状态、响应头与正文；可通过 pm.datasets(ID) 查询工作区数据集。") }><Editor label={t("Mock 实现脚本")} value={script} onChange={onChange} dark={dark} language="javascript" height="180px" /></Field> : <Text size="1" color="gray">{t("在工作区设置中启用 v2 脚本模式后可编辑动态 Mock。")}</Text>;
}
