import { t, useLanguage } from "../../shared/i18n";
import { useEffect, useRef, useState } from "react";
import { Button, Checkbox, Flex, Text, TextField } from "@radix-ui/themes";
import { Plus, Trash2 } from "lucide-react";
import { Choice, Field, ToolButton } from "../../shared/ui";
import { useWorkbench } from "../workbench/context";
import type { MqttConfig, MqttSubscription } from "./types";
import { mqttSubscription } from "./model";
import MqttPropertyEditor from "./MqttPropertyEditor";
export default function MqttSubscriptions({
  value,
  update,
  identity,
}: {
  value: MqttConfig;
  update: (patch: Partial<MqttConfig>) => void;
  identity: string;
}) {
  useLanguage();
  const state = useWorkbench();
  const latest = useRef(state);
  latest.current = state;
  const [draft, setDraft] = useState(() => mqttSubscription());
  const [pending, setPending] = useState(false);
  const scope = useRef(identity);
  scope.current = identity;
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  useEffect(() => {
    setDraft(mqttSubscription());
    setPending(false);
  }, [identity]);
  const session = state.protocolSession.session;
  const active = !!session && ["connecting", "open"].includes(session.state);
  const open = session?.state === "open";
  const blocked = pending || state.protocolSession.sending || (active && !open);
  const v5 = value.version === "5";
  const patch = (changes: Partial<MqttSubscription>) =>
    setDraft((old) => ({ ...old, ...changes }));
  async function apply(
    subscription: MqttSubscription,
    enabled: boolean,
    remove = false,
  ) {
    if (blocked || !subscription.filter.trim()) return;
    const origin = identity;
    const snapshot = { ...structuredClone(subscription), enabled };
    setPending(true);
    try {
      if (
        active &&
        !(await state.protocolSession.send(
          enabled
            ? { kind: "mqtt_subscribe", subscription: snapshot }
            : { kind: "mqtt_unsubscribe", filter: snapshot.filter },
        ))
      )
        return;
      if (!mounted.current || scope.current !== origin) return;
      const current = latest.current.request?.protocol;
      if (current?.kind !== "mqtt") return;
      const index = current.subscriptions.findIndex(
        (item) => item.filter === snapshot.filter,
      );
      const subscriptions = remove
        ? current.subscriptions.filter(
            (item) => item.filter !== snapshot.filter,
          )
        : index < 0
          ? [...current.subscriptions, snapshot]
          : current.subscriptions.map((item, key) =>
              key === index ? snapshot : item,
            );
      update({ subscriptions });
      setDraft(mqttSubscription());
    } finally {
      if (mounted.current && scope.current === origin) setPending(false);
    }
  }
  return (
    <div className="mqtt-subscriptions">
      <Flex gap="3" align="end" wrap="wrap">
        <Field label="Topic Filter">
          <TextField.Root
            value={draft.filter}
            disabled={blocked}
            placeholder="devices/+/temperature"
            onChange={(event) => patch({ filter: event.target.value })}
          />
        </Field>
        <Choice
          value={String(draft.qos)}
          label={t("订阅 QoS")}
          disabled={blocked}
          onChange={(value) => patch({ qos: Number(value) as 0 | 1 | 2 })}
          options={[
            { value: "0", label: "QoS 0" },
            { value: "1", label: "QoS 1" },
            { value: "2", label: "QoS 2" },
          ]}
        />
        <label className="checkbox-label">
          <Checkbox
            checked={draft.filter_secret}
            disabled={blocked}
            onCheckedChange={(value) =>
              patch({ filter_secret: value === true })
            }
          /> {t("私密 Filter")} </label>
        <Button
          disabled={!draft.filter.trim() || blocked}
          loading={pending}
          onClick={() => void apply(draft, true)}
        >
          <Plus size={14} /> {t("添加订阅")} </Button>
      </Flex>
      <Flex gap="3" align="end" wrap="wrap">
        <label className="checkbox-label">
          <Checkbox
            checked={draft.no_local}
            disabled={blocked || !v5}
            onCheckedChange={(value) => patch({ no_local: value === true })}
          />
          No Local
        </label>
        <label className="checkbox-label">
          <Checkbox
            checked={draft.retain_as_published}
            disabled={blocked || !v5}
            onCheckedChange={(value) =>
              patch({ retain_as_published: value === true })
            }
          />
          Retain As Published
        </label>
        <Choice
          value={String(draft.retain_handling)}
          label="Retain Handling"
          disabled={blocked || !v5}
          onChange={(value) =>
            patch({ retain_handling: Number(value) as 0 | 1 | 2 })
          }
          options={[
            { value: "0", label: t("发送保留消息") },
            { value: "1", label: t("仅新订阅发送") },
            { value: "2", label: t("不发送保留消息") },
          ]}
        />
        <Field label="Subscription Identifier">
          <TextField.Root
            type="number"
            min="1"
            value={draft.subscription_identifier ?? ""}
            disabled={blocked || !v5}
            onChange={(event) =>
              patch({
                subscription_identifier:
                  event.target.value === "" ? null : Number(event.target.value),
              })
            }
          />
        </Field>
      </Flex>
      <MqttPropertyEditor
        value={draft.user_properties}
        disabled={blocked || !v5}
        onChange={(user_properties) => patch({ user_properties })}
        label={t("订阅属性")}
      />
      <div
        className="socketio-listeners"
        role="list"
        aria-label={t("MQTT 订阅列表")}
      >
        {value.subscriptions.map((item) => (
          <div key={item.filter} role="listitem" className="socketio-listener">
            <label className="checkbox-label">
              <Checkbox
                checked={item.enabled}
                disabled={blocked}
                onCheckedChange={(enabled) =>
                  void apply(item, enabled === true)
                }
              />
              <Text className="mono">
                {item.filter_secret ? t("[私密 Filter]") : item.filter}
              </Text>
            </label>
            <Flex gap="3" align="center">
              <Text size="1" color="gray">
                QoS {item.qos}
              </Text>
              <ToolButton
                label={
                  t("移除订阅 {{filter}}", { filter: item.filter_secret ? t("私密 Filter") : item.filter })
                }
                disabled={blocked}
                onClick={() => void apply(item, false, true)}
              >
                <Trash2 size={14} />
              </ToolButton>
            </Flex>
          </div>
        ))}
        {!value.subscriptions.length && (
          <Text color="gray"> {t("支持精确 Topic 和 broker 允许的通配符过滤器。")} </Text>
        )}
      </div>
    </div>
  );
}
