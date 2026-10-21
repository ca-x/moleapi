import NetworkSettings from "../network/NetworkSettings";
import { t, useLanguage } from "../../shared/i18n";
import { useEffect, useRef, useState } from "react";
import {
  AlertDialog,
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
import SessionEventPane from "../protocols/SessionEventPane";
import type { RequestSpec } from "../../shared/types";
import type { ProtocolEvent, ProtocolSession, SendMessage } from "../protocols/types";
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
export interface MqttChannel {
  session: ProtocolSession | null; events: ProtocolEvent[]; dropped: number; error: string; busy: boolean; sending: boolean;
  send: (message: SendMessage) => Promise<boolean>; close: () => Promise<void>;
}
const idle: MqttChannel = { session: null, events: [], dropped: 0, error: "", busy: false, sending: false, send: async () => false, close: async () => {} };
export default function MqttClient(state: {
  scope: string; request: RequestSpec; dark: boolean; onChange: (patch: Partial<RequestSpec>) => void; isCurrent: () => boolean;
  channel?: MqttChannel; disabled?: boolean; connectionSettings?: boolean; savedMessagesEditable?: boolean; networkSettings?: boolean;
}) {
  useLanguage();
  const latest = useRef(state); latest.current = state;
  const config = state.request.protocol?.kind === "mqtt" ? state.request.protocol : null;
  const { session, events, dropped, error, busy, sending } = state.channel ?? idle;
  const connectionSettings = state.connectionSettings !== false;
  const savedMessagesEditable = state.savedMessagesEditable !== false;
  const identity = JSON.stringify([state.scope, session?.id]);
  const scope = useRef(identity); scope.current = identity;
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const [abortFor, setAbortFor] = useState<string | null>(null);
  function admitted(origin = identity) { return mounted.current && scope.current === origin && !latest.current.disabled && latest.current.isCurrent(); }
  async function close() { if (admitted()) await latest.current.channel?.close(); }
  async function publish() {
    const value = latest.current.request.protocol, channel = latest.current.channel;
    if (admitted() && channel?.session?.state === "open" && !channel.sending && value?.kind === "mqtt" && value.message.topic && !(value.version !== "5" && hasV5Settings(value))) await channel.send({ kind: "mqtt_publish", message: structuredClone(value.message) });
  }
  async function abort() {
    const origin = abortFor, channel = latest.current.channel;
    setAbortFor(null);
    if (origin === identity && admitted(origin) && channel?.session && ["connecting", "open"].includes(channel.session.state) && !channel.sending) await channel.send({ kind: "mqtt_abort" });
  }
  const [tab, setTab] = useState("publish");
  const [messageName, setMessageName] = useState("");
  const [savedId, setSavedId] = useState("none");
  useEffect(() => { setTab("publish"); setMessageName(""); setSavedId("none"); setAbortFor(null); }, [identity]);
  function update(patch: Partial<MqttConfig>) {
    const current = latest.current.request?.protocol;
    if (admitted() && current?.kind === "mqtt")
      latest.current.onChange({ protocol: { ...current, ...patch } });
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
    if (!value || !admitted() || !savedMessagesEditable) return;
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
              disabled={sending || state.disabled}
              onClick={() => { if (admitted()) setAbortFor(identity); }}
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
            disabled={active || !!state.disabled}
            onClick={() => update(clearV5Settings(config))}
          > {t("清除 MQTT 5 专有设置")} </Button>
        </Callout.Root>
      )}
      <Tabs.Root value={tab} onValueChange={setTab} className="grpc-tabs">
        <Tabs.List>
          <Tabs.Trigger value="publish">{t("发布消息")}</Tabs.Trigger>
          {state.channel && <Tabs.Trigger value="messages"> {t("消息与状态")} <span className="count">{events.length}</span>
          </Tabs.Trigger>}
          <Tabs.Trigger value="topics">Topics</Tabs.Trigger>
          {connectionSettings && <Tabs.Trigger value="settings">{t("连接设置")}</Tabs.Trigger>}
          {connectionSettings && <Tabs.Trigger value="will">Last Will</Tabs.Trigger>}
          {state.channel && <Tabs.Trigger value="telemetry">{t("遥测图表")}</Tabs.Trigger>}
        </Tabs.List>
        <Tabs.Content value="publish">
          <Flex gap="3" align="end" wrap="wrap">
            <Choice
              value={saved?.id || "none"}
              disabled={state.disabled}
              label={t("保存的 MQTT 消息")}
              onChange={(value) => {
                if (!admitted()) return;
                setSavedId(value);
                const entry = current()?.saved_messages.find(
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
            {savedMessagesEditable && <><Field label={t("消息名称")}>
              <TextField.Root
                disabled={state.disabled}
                value={messageName}
                onChange={(event) => { if (admitted()) setMessageName(event.target.value); }}
                placeholder={t("保存以便复用")}
              />
            </Field>
            <Button
              size="1"
              color="gray"
              variant="soft"
              disabled={state.disabled}
              onClick={() => saveMessage()}
            >
              <Save size={14} /> {t("保存新消息")} </Button>
            {saved && (
              <>
                <Button
                  size="1"
                  color="gray"
                  variant="outline"
                  disabled={state.disabled}
                  onClick={() => saveMessage(true)}
                > {t("更新已选消息")} </Button>
                <Button
                  size="1"
                  color="gray"
                  variant="ghost"
                  disabled={state.disabled}
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
            </>}
            {state.channel && <Button disabled={state.disabled || !open || !config.message.topic || incompatible || sending} loading={sending} onClick={() => void publish()}><Send size={15} /> {t("发布")}</Button>}
          </Flex>
          <MqttMessageEditor
            value={config.message}
            onChange={setMessage}
            disabled={state.disabled}
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
            channel={state.channel}
            disabled={state.disabled}
            isCurrent={admitted}
          />
        </Tabs.Content>
        {connectionSettings && <Tabs.Content value="settings">
          {state.networkSettings !== false && <NetworkSettings key={identity} mqtt value={state.request.network} change={network=>{if(admitted()) latest.current.onChange({network});}} disabled={active||busy||state.disabled}/>}
          <MqttConnection
            value={config}
            onChange={(value) => update(value)}
            disabled={active || !!state.disabled}
          />
        </Tabs.Content>}
        {connectionSettings && <Tabs.Content value="will">
          <label className="checkbox-label">
            <Checkbox
              checked={!!config.will}
              disabled={active || !!state.disabled}
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
                  disabled={active || state.disabled || !v5}
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
                disabled={active || !!state.disabled}
                v5={v5}
                label="Last Will"
                showProperties
              />
            </>
          )}
          <Text size="2" color="gray"> {t("正常断开会发送 DISCONNECT；中止或异常断开时，broker 决定是否及何时发布遗嘱。")} </Text>
        </Tabs.Content>}
        <Tabs.Content value="telemetry">
          <MqttTelemetry events={events} sessionId={session?.id} />
        </Tabs.Content>
      </Tabs.Root>
      <AlertDialog.Root open={abortFor === identity} onOpenChange={(open) => { if (!open) setAbortFor(null); }}>
        <AlertDialog.Content><AlertDialog.Title>{t("中止 MQTT 连接")}</AlertDialog.Title><AlertDialog.Description>{t("不发送 DISCONNECT。Broker 可能根据 Last Will 和延迟设置发布遗嘱消息。")}</AlertDialog.Description><Flex gap="3" mt="4" justify="end"><AlertDialog.Cancel><Button color="gray" variant="soft">{t("取消")}</Button></AlertDialog.Cancel><Button color="red" disabled={state.disabled || sending} onClick={() => void abort()}>{t("中止连接")}</Button></Flex></AlertDialog.Content>
      </AlertDialog.Root>
    </section>
  );
}
