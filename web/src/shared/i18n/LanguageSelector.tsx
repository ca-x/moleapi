import { Text } from "@radix-ui/themes";
import { Choice, Field } from "../ui";
import { t, useLanguage } from "./index";
export default function LanguageSelector() {
  const { language, setLanguage } = useLanguage();
  return (
    <Field label={t("界面语言")}>
      <Choice
        label={t("界面语言")}
        value={language}
        onChange={(value) => void setLanguage(value)}
        options={[{ value: "zh-CN", label: "简体中文" }, { value: "en", label: "English" }]}
      />
      <Text size="1" color="gray">{t("语言选择立即生效，仅保存在当前设备。")}</Text>
    </Field>
  );
}
