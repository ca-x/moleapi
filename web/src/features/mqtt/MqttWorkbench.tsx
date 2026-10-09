import NetworkSettings from "../network/NetworkSettings";
import { t, useLanguage, message } from "../../shared/i18n";
import { useEffect, useRef, useState } from "react";
import {
  Badge,
  Button,
  Callout,
  Checkbox,
  Flex,
  Tabs,
  Text,
  TextField,
} from "@radix-ui/themes";
import { Save, Send, Square, Trash2, Zap } from "lucide-react";
import { Choice, Field } from "../../shared/ui";
import { bytes, id } from "../../shared/model";
import { useWorkbench } from "../workbench/context";
import SessionEventPane from "../protocols/SessionEventPane";
import type { MqttConfig, MqttMessage } from "./types";
import {
  clearV5Settings,
  hasV5Settings,
  mqttMessage,
  mqttSavedLabel,
} from "./model";
import MqttConnection from "./MqttConnection";
import MqttMessageEditor from "./MqttMessageEditor";
import MqttSubscriptions from "./MqttSubscriptions";
import MqttTelemetry from "./MqttTelemetry";
export default function MqttWorkbench() {
  useLanguage();
  const state = useWorkbench();
  const latest = useRef(state);
  latest.current = state;
  const config =
    state.request?.protocol?.kind === "mqtt" ? state.request.protocol : null;
  const { session, events, dropped, error, busy, sending, send, close } =
    state.protocolSession;
  const [tab, setTab] = useState("publish");
  const [messageName, setMessageName] = useState("");
  const [savedId, setSavedId] = useState("none");
  const identity = JSON.stringify([
    state.authenticated,
    state.accountId,
    state.draft?.id,
    state.request?.id,
    state.draft?.data.active_environment_id,
  ]);
  useEffect(() => {
    setTab("publish");
    setMessageName("");
    setSavedId("none");
  }, [state.accountId, state.draft?.id, state.request?.id]);
  function update(patch: Partial<MqttConfig>) {
    const current = latest.current.request?.protocol;
    if (current?.kind === "mqtt")
      latest.current.updateRequest({ protocol: { ...current, ...patch } });
  }
  function current(): MqttConfig | null {
    const value = latest.current.request?.protocol;
    return value?.kind === "mqtt" ? value : null;
  }
  function setMessage(message: MqttMessage) {
    update({ message });
  }
  function saveMessage(replace = false) {
    const value = current();
    if (!value) return;
    const existing = value.saved_messages.find((item) => item.id === savedId);
    const safeLabel = mqttSavedLabel(
      messageName,
      existing?.name,
      value.message,
    );
    const entry = {
      id: replace && existing ? existing.id : id(),
      name: safeLabel,
      message: structuredClone(value.message),
    };
    update({
      saved_messages:
        replace && existing
          ? value.saved_messages.map((item) =>
              item.id === existing.id ? entry : item,
            )
          : [...value.saved_messages, entry],
    });
    setSavedId(entry.id);
    setMessageName(entry.name);
  }
  if (!config) return null;
  const active = !!session && ["connecting", "open"].includes(session.state);
  const open = session?.state === "open";
  const v5 = config.version === "5";
  const saved = config.saved_messages.find((item) => item.id === savedId);
  const incompatible = !v5 && hasV5Settings(config);
  const statuses = events.filter(
    (event) => event.message.kind === "mqtt_status",
  );
  const lastStatus = statuses.at(-1)?.message;
  return (
    <section className="mqtt-workbench" aria-label={t("MQTT 客户端")}>
      <div className="mqtt-toolbar">
        <Flex gap="3" align="center" wrap="wrap">
          <Text size="2" weight="medium">
            MQTT {config.version}
          </Text>
          {session && (
            <Badge
              color={
                open ? "green" : session.state === "error" ? "red" : "gray"
              }
            >
              {session.state}
            </Badge>
          )}
          {lastStatus?.kind === "mqtt_status" && (
            <Text size="1" color="gray">
              {lastStatus.operation + " · " + lastStatus.status}
            </Text>
          )}
          {session && (
            <Text size="1" color="gray" className="mono">
              ↓ {bytes(session.received_bytes)} · ↑ {bytes(session.sent_bytes)}
            </Text>
          )}
          {(active || busy) && (
            <Button
              size="1"
              variant="soft"
              color="gray"
              onClick={() => void close()}
            >
              <Square size={14} />
              {busy ? t("取消连接") : t("正常断开")}
            </Button>
          )}
          {active && (
            <Button
              size="1"
              variant="outline"
              color="gray"
              disabled={sending}
              onClick={() =>
                state.setGuard({
                  title: message("中止 MQTT 连接"),
                  description:
                    message("不发送 DISCONNECT。Broker 可能根据 Last Will 和延迟设置发布遗嘱消息。"),
                  action: () => {
                    void send({ kind: "mqtt_abort" });
                  },
                })
              }
            >
              <Zap size={14} /> {t("中止连接")} </Button>
          )}
        </Flex>
      </div>
      {error && (
        <Callout.Root color="red" role="alert">
          <Callout.Text>{error}</Callout.Text>
        </Callout.Root>
      )}
      {session?.reason && !active && (
        <Text size="1" color="gray" className="grpc-reason">
          {session.reason}
        </Text>
      )}
      {incompatible && (
        <Callout.Root color="amber">
          <Callout.Text> {t("当前保存了 MQTT 5 专有属性。切回 MQTT 5 使用，或清除后再连接 MQTT 3.1.1。")} </Callout.Text>
          <Button
            size="1"
            variant="soft"
            color="gray"
            disabled={active}
            onClick={() => update(clearV5Settings(config))}
          > {t("清除 MQTT 5 专有设置")} </Button>
        </Callout.Root>
      )}
      <Tabs.Root value={tab} onValueChange={setTab} className="grpc-tabs">
        <Tabs.List>
          <Tabs.Trigger value="publish">{t("发布消息")}</Tabs.Trigger>
          <Tabs.Trigger value="messages"> {t("消息与状态")} <span className="count">{events.length}</span>
          </Tabs.Trigger>
          <Tabs.Trigger value="topics">Topics</Tabs.Trigger>
          <Tabs.Trigger value="settings">{t("连接设置")}</Tabs.Trigger>
          <Tabs.Trigger value="will">Last Will</Tabs.Trigger>
          <Tabs.Trigger value="telemetry">{t("遥测图表")}</Tabs.Trigger>
        </Tabs.List>
        <Tabs.Content value="publish">
          <Flex gap="3" align="end" wrap="wrap">
            <Choice
              value={saved?.id || "none"}
              label={t("保存的 MQTT 消息")}
              onChange={(value) => {
                setSavedId(value);
                const entry = config.saved_messages.find(
                  (item) => item.id === value,
                );
                if (entry) {
                  setMessage(structuredClone(entry.message));
                  setMessageName(entry.name);
                }
              }}
              options={[
                { value: "none", label: t("当前编辑消息") },
                ...config.saved_messages.map((item) => ({
                  value: item.id,
                  label: item.name,
                })),
              ]}
            />
            <Field label={t("消息名称")}>
              <TextField.Root
                value={messageName}
                onChange={(event) => setMessageName(event.target.value)}
                placeholder={t("保存以便复用")}
              />
            </Field>
            <Button
              size="1"
              color="gray"
              variant="soft"
              onClick={() => saveMessage()}
            >
              <Save size={14} /> {t("保存新消息")} </Button>
            {saved && (
              <>
                <Button
                  size="1"
                  color="gray"
                  variant="outline"
                  onClick={() => saveMessage(true)}
                > {t("更新已选消息")} </Button>
                <Button
                  size="1"
                  color="gray"
                  variant="ghost"
                  onClick={() => {
                    update({
                      saved_messages: config.saved_messages.filter(
                        (item) => item.id !== saved.id,
                      ),
                    });
                    setSavedId("none");
                  }}
                >
                  <Trash2 size={14} /> {t("移除")} </Button>
              </>
            )}
            <Button
              disabled={!open || !config.message.topic || incompatible}
              loading={sending}
              onClick={() =>
                void send({
                  kind: "mqtt_publish",
                  message: structuredClone(config.message),
                })
              }
            >
              <Send size={15} /> {t("发布")} </Button>
          </Flex>
          <MqttMessageEditor
            value={config.message}
            onChange={setMessage}
            dark={state.dark}
            v5={v5}
            showProperties
          />
          <Text size="1" color="gray"> {t("已入队、已发送、QoS 确认由 SDK 事件显示；broker 确认不代表业务处理完成。")} </Text>
        </Tabs.Content>
        <Tabs.Content value="messages">
          <SessionEventPane
            protocolLabel="MQTT"
            events={events}
            dropped={dropped}
            dark={state.dark}
            sessionId={session?.id}
          />
        </Tabs.Content>
        <Tabs.Content value="topics">
          <MqttSubscriptions
            value={config}
            update={update}
            identity={identity}
          />
        </Tabs.Content>
        <Tabs.Content value="settings">
          <NetworkSettings key={identity} mqtt value={state.request?.network} change={network=>state.updateRequest({network})} disabled={active||busy}/>
          <MqttConnection
            value={config}
            onChange={(value) => update(value)}
            disabled={active}
          />
        </Tabs.Content>
        <Tabs.Content value="will">
          <label className="checkbox-label">
            <Checkbox
              checked={!!config.will}
              disabled={active}
              onCheckedChange={(value) =>
                update({
                  will:
                    value === true
                      ? { message: mqttMessage(), delay_interval: 0 }
                      : null,
                })
              }
            /> {t("启用 Last Will")} </label>
          {config.will && (
            <>
              <Field label={t("Will Delay（秒）")}>
                <TextField.Root
                  type="number"
                  min="0"
                  value={config.will.delay_interval}
                  disabled={active || !v5}
                  onChange={(event) =>
                    update({
                      will: {
                        ...config.will!,
                        delay_interval: Number(event.target.value),
                      },
                    })
                  }
                />
              </Field>
              <MqttMessageEditor
                value={config.will.message}
                onChange={(message) =>
                  update({ will: { ...config.will!, message } })
                }
                dark={state.dark}
                disabled={active}
                v5={v5}
                label="Last Will"
                showProperties
              />
            </>
          )}
          <Text size="2" color="gray"> {t("正常断开会发送 DISCONNECT；中止或异常断开时，broker 决定是否及何时发布遗嘱。")} </Text>
        </Tabs.Content>
        <Tabs.Content value="telemetry">
          <MqttTelemetry events={events} sessionId={session?.id} />
        </Tabs.Content>
      </Tabs.Root>
    </section>
  );
}
