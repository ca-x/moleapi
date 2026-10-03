import { Checkbox, Flex, Text, TextField } from "@radix-ui/themes";
import { Choice, Editor, Field } from "../../shared/ui";
import MqttPropertyEditor from "./MqttPropertyEditor";
import type { MqttMessage } from "./types";
export default function MqttMessageEditor({
  value,
  onChange,
  dark,
  disabled = false,
  v5 = true,
  label = "MQTT 消息",
  showProperties = false,
}: {
  value: MqttMessage;
  onChange: (value: MqttMessage) => void;
  dark: boolean;
  disabled?: boolean;
  v5?: boolean;
  label?: string;
  showProperties?: boolean;
}) {
  const update = (patch: Partial<MqttMessage>) =>
    onChange({ ...value, ...patch });
  const properties = value.properties;
  return (
    <div className="mqtt-message-editor">
      <Flex gap="3" align="end" wrap="wrap">
        <Field label={label + " Topic"}>
          <TextField.Root
            value={value.topic}
            disabled={disabled}
            placeholder="devices/demo/temperature"
            onChange={(event) => update({ topic: event.target.value })}
          />
        </Field>
        <Choice
          value={String(value.qos)}
          label={label + " QoS"}
          disabled={disabled}
          onChange={(value) => update({ qos: Number(value) as 0 | 1 | 2 })}
          options={[
            { value: "0", label: "QoS 0" },
            { value: "1", label: "QoS 1" },
            { value: "2", label: "QoS 2" },
          ]}
        />
        <Choice
          value={value.encoding}
          label={label + " 编码"}
          disabled={disabled}
          onChange={(encoding) => update({ encoding })}
          options={[
            { value: "text", label: "Text" },
            { value: "json", label: "JSON" },
            { value: "base64", label: "Binary / Base64" },
          ]}
        />
        <label className="checkbox-label">
          <Checkbox
            checked={value.retain}
            disabled={disabled}
            onCheckedChange={(value) => update({ retain: value === true })}
          />
          Retain
        </label>
      </Flex>
      <Flex gap="4" wrap="wrap">
        <label className="checkbox-label">
          <Checkbox
            checked={value.topic_secret}
            disabled={disabled}
            onCheckedChange={(value) =>
              update({ topic_secret: value === true })
            }
          />
          私密 Topic
        </label>
        <label className="checkbox-label">
          <Checkbox
            checked={value.payload_secret}
            disabled={disabled}
            onCheckedChange={(value) =>
              update({ payload_secret: value === true })
            }
          />
          私密 Payload
        </label>
      </Flex>
      <Editor
        value={value.payload_source}
        onChange={(payload_source) => update({ payload_source })}
        readOnly={disabled}
        dark={dark}
        jsonMode={value.encoding === "json"}
        height="240px"
        label={label + " Payload"}
      />
      {value.encoding === "base64" && (
        <Text size="1" color="gray">
          填写 Base64；空字符串表示零字节消息，可配合 Retain 清除 broker
          保留值。
        </Text>
      )}
      {showProperties && (
        <div className="mqtt-message-properties">
          <Text as="p" size="2" weight="medium">
            MQTT 5 消息属性
          </Text>
          <Flex gap="3" wrap="wrap">
            <Field label="Content Type">
              <TextField.Root
                value={properties.content_type || ""}
                disabled={disabled || !v5}
                onChange={(event) =>
                  update({
                    properties: {
                      ...properties,
                      content_type: event.target.value || null,
                    },
                  })
                }
              />
            </Field>
            <Field label="Response Topic">
              <TextField.Root
                value={properties.response_topic || ""}
                disabled={disabled || !v5}
                onChange={(event) =>
                  update({
                    properties: {
                      ...properties,
                      response_topic: event.target.value || null,
                    },
                  })
                }
              />
            </Field>
            <Field label="Correlation Data（Base64）">
              <TextField.Root
                value={properties.correlation_data_base64 || ""}
                disabled={disabled || !v5}
                onChange={(event) =>
                  update({
                    properties: {
                      ...properties,
                      correlation_data_base64: event.target.value || null,
                    },
                  })
                }
              />
            </Field>
            <Field label="Message Expiry（秒）">
              <TextField.Root
                type="number"
                value={properties.message_expiry_interval ?? ""}
                disabled={disabled || !v5}
                min="0"
                onChange={(event) =>
                  update({
                    properties: {
                      ...properties,
                      message_expiry_interval:
                        event.target.value === ""
                          ? null
                          : Number(event.target.value),
                    },
                  })
                }
              />
            </Field>
            <Choice
              value={
                properties.payload_format_indicator === null
                  ? "none"
                  : String(properties.payload_format_indicator)
              }
              label="Payload Format"
              disabled={disabled || !v5}
              onChange={(value) =>
                update({
                  properties: {
                    ...properties,
                    payload_format_indicator:
                      value === "none" ? null : (Number(value) as 0 | 1),
                  },
                })
              }
              options={[
                { value: "none", label: "未设置" },
                { value: "0", label: "Binary" },
                { value: "1", label: "UTF-8" },
              ]}
            />
          </Flex>
          <MqttPropertyEditor
            value={properties.user_properties}
            disabled={disabled || !v5}
            onChange={(user_properties) =>
              update({ properties: { ...properties, user_properties } })
            }
            label={label + " Properties"}
          />
        </div>
      )}
    </div>
  );
}
