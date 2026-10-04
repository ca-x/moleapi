import { t, useLanguage } from "../../shared/i18n";
import { Checkbox, Flex, Text, TextField } from "@radix-ui/themes";
import { Choice, Field } from "../../shared/ui";
import MqttPropertyEditor from "./MqttPropertyEditor";
import type { MqttConfig } from "./types";
export default function MqttConnection({
  value,
  onChange,
  disabled,
}: {
  value: MqttConfig;
  onChange: (value: MqttConfig) => void;
  disabled: boolean;
}) {
  useLanguage();
  const change = (patch: Partial<MqttConfig>) =>
    onChange({ ...value, ...patch });
  const v5 = value.version === "5";
  return (
    <div className="mqtt-settings">
      <Flex gap="3" wrap="wrap" align="end">
        <Choice
          value={value.version}
          label={t("MQTT 版本")}
          disabled={disabled}
          onChange={(version) => change({ version })}
          options={[
            { value: "5", label: "MQTT 5" },
            { value: "3.1.1", label: "MQTT 3.1.1" },
          ]}
        />
        <Field label="Client ID">
          <TextField.Root
            value={value.client_id}
            disabled={disabled}
            placeholder={t("留空为本次连接生成 ID")}
            onChange={(event) => change({ client_id: event.target.value })}
          />
        </Field>
        <Field label={t("Keep Alive（秒）")}>
          <TextField.Root
            type="number"
            min="5"
            max="3600"
            value={value.keep_alive_secs}
            disabled={disabled}
            onChange={(event) =>
              change({ keep_alive_secs: Number(event.target.value) })
            }
          />
        </Field>
        <label className="checkbox-label">
          <Checkbox
            checked={value.clean_start}
            disabled={disabled}
            onCheckedChange={(value) => change({ clean_start: value === true })}
          />
          Clean Start / Session
        </label>
        <Field label={t("Session Expiry（秒）")}>
          <TextField.Root
            type="number"
            min="0"
            value={value.session_expiry_interval}
            disabled={disabled || !v5}
            onChange={(event) =>
              change({ session_expiry_interval: Number(event.target.value) })
            }
          />
        </Field>
      </Flex>
      <Text size="1" color="gray"> {t("持久会话使用固定 Client ID；留空生成的 ID 不会写入共享请求。")} </Text>
      <Flex gap="3" align="end" wrap="wrap">
        <label className="checkbox-label">
          <Checkbox
            checked={value.reconnect.enabled}
            disabled={disabled}
            onCheckedChange={(enabled) =>
              change({
                reconnect: { ...value.reconnect, enabled: enabled === true },
              })
            }
          /> {t("自动重连")} </label>
        <Field label={t("最大重连次数")}>
          <TextField.Root
            type="number"
            min="1"
            max="10"
            value={value.reconnect.max_attempts}
            disabled={disabled || !value.reconnect.enabled}
            onChange={(event) =>
              change({
                reconnect: {
                  ...value.reconnect,
                  max_attempts: Number(event.target.value),
                },
              })
            }
          />
        </Field>
        <Field label={t("重连间隔（ms）")}>
          <TextField.Root
            type="number"
            min="100"
            max="60000"
            value={value.reconnect.delay_ms}
            disabled={disabled || !value.reconnect.enabled}
            onChange={(event) =>
              change({
                reconnect: {
                  ...value.reconnect,
                  delay_ms: Number(event.target.value),
                },
              })
            }
          />
        </Field>
      </Flex>
      <Text as="p" size="2" weight="medium"> {t("连接 User Properties")} </Text>
      <MqttPropertyEditor
        value={value.user_properties}
        disabled={disabled || !v5}
        onChange={(user_properties) => change({ user_properties })}
        label={t("连接属性")}
      />
      <Text size="2" color="gray"> {t("匿名连接使用 None；用户名／密码在上方鉴权中选择 Basic。")} </Text>
    </div>
  );
}
