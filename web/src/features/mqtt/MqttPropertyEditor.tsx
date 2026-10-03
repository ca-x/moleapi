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
            aria-label={label + " 名称 " + (index + 1)}
            value={item.key}
            disabled={disabled}
            placeholder="名称"
            onChange={(event) => edit(index, { key: event.target.value })}
          />
          <TextField.Root
            aria-label={label + " 值 " + (index + 1)}
            value={item.value}
            type={item.secret ? "password" : "text"}
            disabled={disabled}
            placeholder="值"
            onChange={(event) => edit(index, { value: event.target.value })}
          />
          <Checkbox
            aria-label={label + " 密钥 " + (index + 1)}
            checked={item.secret}
            disabled={disabled}
            onCheckedChange={(value) => edit(index, { secret: value === true })}
          />
          <ToolButton
            label={"移除 " + label + " " + (index + 1)}
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
        <Plus size={14} />
        添加属性
      </Button>
    </div>
  );
}
