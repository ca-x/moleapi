import { t, useLanguage } from "../../shared/i18n";
import { Button, Checkbox, Flex, TextField } from "@radix-ui/themes";
import { Plus, Trash2 } from "lucide-react";
import { ToolButton } from "../../shared/ui";
import type { MqttProperty } from "./types";
export default function MqttPropertyEditor({
  value,
  onChange,
  disabled = false,
  label = "User Properties",
}: {
  value: MqttProperty[];
  onChange: (value: MqttProperty[]) => void;
  disabled?: boolean;
  label?: string;
}) {
  useLanguage();
  function edit(index: number, patch: Partial<MqttProperty>) {
    onChange(
      value.map((item, key) => (key === index ? { ...item, ...patch } : item)),
    );
  }
  return (
    <div className="mqtt-property-editor" aria-label={label}>
      {value.map((item, index) => (
        <Flex key={index} gap="2" align="center">
          <TextField.Root
            aria-label={t("{{label}} 名称 {{number}}", { label, number: index + 1 })}
            value={item.key}
            disabled={disabled}
            placeholder={t("名称")}
            onChange={(event) => edit(index, { key: event.target.value })}
          />
          <TextField.Root
            aria-label={t("{{label}} 值 {{number}}", { label, number: index + 1 })}
            value={item.value}
            type={item.secret ? "password" : "text"}
            disabled={disabled}
            placeholder={t("值")}
            onChange={(event) => edit(index, { value: event.target.value })}
          />
          <Checkbox
            aria-label={t("{{label}} 密钥 {{number}}", { label, number: index + 1 })}
            checked={item.secret}
            disabled={disabled}
            onCheckedChange={(value) => edit(index, { secret: value === true })}
          />
          <ToolButton
            label={t("移除 {{label}} {{number}}", { label, number: index + 1 })}
            disabled={disabled}
            onClick={() => onChange(value.filter((_, key) => key !== index))}
          >
            <Trash2 size={14} />
          </ToolButton>
        </Flex>
      ))}
      <Button
        size="1"
        variant="soft"
        color="gray"
        disabled={disabled}
        onClick={() =>
          onChange([...value, { key: "", value: "", secret: false }])
        }
      >
        <Plus size={14} /> {t("添加属性")} </Button>
    </div>
  );
}
