import { t, useLanguage } from "../../shared/i18n";
import { useEffect, useRef, useState } from "react";
import { Badge, Button, Callout, Flex, Tabs, Text } from "@radix-ui/themes";
import {
  Braces,
  FileCode,
  RefreshCw,
  Send,
  Square,
  StepForward,
} from "lucide-react";
import { Choice, Editor } from "../../shared/ui";
import { bytes } from "../../shared/model";
import type { GrpcConfig, RequestSpec, Specification } from "../../shared/types";
import type { ProtocolSession, ProtocolEvent, SendMessage } from "../protocols/types";
import type { GrpcSchema } from "./types";
import { methodMode } from "./types";
import GrpcEvents from "./GrpcEvents";
export interface GrpcChannel { session: ProtocolSession | null; events: ProtocolEvent[]; dropped: number; error: string; busy: boolean; sending: boolean; send: (message: SendMessage) => Promise<boolean>; close: () => Promise<void>; }
const idle: GrpcChannel = { session: null, events: [], dropped: 0, error: "", busy: false, sending: false, send: async () => false, close: async () => {} };
export default function GrpcClient(state: {
  scope: string; request: RequestSpec; specifications: Specification[]; schema: GrpcSchema | null; dark: boolean; onChange: (patch: Partial<RequestSpec>) => void; isCurrent: () => boolean;
  sourceError?: string; sourceBusy?: boolean; sourceActionsDisabled?: boolean; onImport?: () => void; onReflect?: () => void; channel?: GrpcChannel; connectionSettings?: boolean; disabled?: boolean; editorHeight?: string;
}) {
  useLanguage();
  const latest = useRef(state); latest.current = state;
  const { session, events, dropped, error, busy, sending, close } = state.channel ?? idle;
  const connectionSettings = state.connectionSettings !== false;
  const mounted = useRef(true); useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const [halfClosedFor, setHalfClosedFor] = useState<string | null>(null);
  const [tab, setTab] = useState("message");
  const identity = JSON.stringify([state.scope, session?.id]);
  const scope = useRef(identity); scope.current = identity;
  useEffect(() => {
    setHalfClosedFor(null);
    setTab("message");
  }, [identity]);
  const grpc =
    state.request?.protocol?.kind === "grpc" ? state.request.protocol : null;
  const service = state.schema?.services.find(
    (item) => item.name === grpc?.service,
  );
  const method = service?.methods.find((item) => item.name === grpc?.method);
  const active = !!session && ["connecting", "open"].includes(session.state);
  const halfClosed =
    session?.client_half_closed || (!!session && halfClosedFor === session.id);
  const status = [...events]
    .reverse()
    .find((event) => event.message.kind === "grpc_status")?.message;
  function current(origin = identity) { return mounted.current && scope.current === origin && latest.current.scope === state.scope && !latest.current.disabled && latest.current.isCurrent(); }
  function change(patch: Partial<RequestSpec>) { if (current()) latest.current.onChange(patch); }
  function update(patch: Partial<GrpcConfig>) { const config = latest.current.request.protocol; if (config?.kind === "grpc") change({ protocol: { ...config, ...patch } }); }
  function canSend() { return current() && latest.current.channel?.session?.state === "open" && !latest.current.channel.sending && !latest.current.channel.session.client_half_closed && halfClosedFor !== latest.current.channel.session.id; }
  async function transmit() { const config = latest.current.request.protocol; if (canSend() && config?.kind === "grpc" && method?.client_streaming) await latest.current.channel!.send({ kind: "grpc_message", message_source: config.message_source }); }
  async function halfClose() { if (!canSend() || !method?.client_streaming) return; const origin = identity, id = session!.id; if ((await latest.current.channel!.send({ kind: "grpc_half_close" })) && current(origin)) setHalfClosedFor(id); }
  if (!grpc) return null;
  return (
    <section className="grpc-workbench" aria-label={t("gRPC 客户端")}>
      <div className="grpc-definition-toolbar">
        {connectionSettings && <>
        <Flex gap="3" align="center" wrap="wrap">
          <Choice
            value={state.request?.specification_id || "none"}
            onChange={(value) => { const config = latest.current.request.protocol; if (config?.kind === "grpc") change({ specification_id: value === "none" ? null : value, protocol: { ...config, service: "", method: "" } }); }}
            label={t("gRPC 服务定义")}
            disabled={active || state.disabled}
            options={[
              { value: "none", label: t("选择服务定义") },
              ...state.specifications
                .filter((item) => item.kind === "protobuf")
                .map((item) => ({ value: item.id, label: item.name })),
            ]}
          />
          <Button
            size="1"
            variant="soft"
            color="gray"
            disabled={active || state.disabled || state.sourceActionsDisabled}
            onClick={() => { if (current()) state.onImport?.(); }}
          >
            <FileCode size={14} />
            {state.request.specification_id ? t("Proto 文件") : t("导入 Proto")}
          </Button>
          <Button
            size="1"
            variant="soft"
            color="gray"
            loading={state.sourceBusy}
            disabled={active || state.disabled || state.sourceActionsDisabled || !state.request.url}
            onClick={() => { if (current()) state.onReflect?.(); }}
          >
            <RefreshCw size={14} />
            Server Reflection
          </Button>
        </Flex>
        </>}
        <Flex gap="3" align="center" wrap="wrap" mt="3">
          {connectionSettings && <>
          <Choice
            value={grpc.service || "none"}
            label={t("gRPC 服务")}
            disabled={active || state.disabled || !state.schema}
            options={[
              { value: "none", label: t("选择服务") },
              ...(state.schema?.services || []).map((item) => ({
                value: item.name,
                label: item.name,
              })),
            ]}
            onChange={(value) =>
              update({ service: value === "none" ? "" : value, method: "" })
            }
          />
          <Choice
            value={grpc.method || "none"}
            label={t("gRPC 方法")}
            disabled={active || state.disabled || !service}
            options={[
              { value: "none", label: t("选择方法") },
              ...(service?.methods || []).map((item) => ({
                value: item.name,
                label: item.name,
              })),
            ]}
            onChange={(value) =>
              update({ method: value === "none" ? "" : value })
            }
          />
          </>}
          {method && <Badge color="gray">{methodMode(method)}</Badge>}
          {status?.kind === "grpc_status" && (
            <Badge color={status.code === 0 ? "green" : "red"}>
              {status.code} {status.name}
            </Badge>
          )}
          {session && (
            <Text size="1" color="gray" className="mono">
              ↓ {bytes(session.received_bytes)} · ↑ {bytes(session.sent_bytes)}
            </Text>
          )}
          {connectionSettings && (active || busy) && (
            <Button
              size="1"
              variant="soft"
              color="gray"
              onClick={() => void close()}
            >
              <Square size={14} />
              {busy ? t("取消调用") : t("停止")}
            </Button>
          )}
        </Flex>
      </div>
      {(state.sourceError || error) && (
        <Callout.Root color="red" role="alert">
          <Callout.Text>{state.sourceError || error}</Callout.Text>
        </Callout.Root>
      )}
      {!active && session?.reason && (
        <Text size="1" color="gray" className="grpc-reason">
          {session.reason}
        </Text>
      )}
      <Tabs.Root value={tab} onValueChange={setTab} className="grpc-tabs">
        <Tabs.List>
          <Tabs.Trigger value="message">{t("请求消息")}</Tabs.Trigger>
          {state.channel && <Tabs.Trigger value="events"> {t("消息与状态")} <span className="count">{events.length}</span></Tabs.Trigger>}
          <Tabs.Trigger value="definition">{t("方法定义")}</Tabs.Trigger>
        </Tabs.List>
        <Tabs.Content value="message">
          <div className="grpc-input-toolbar">
            <Text size="1" color="gray" className="mono">
              {method?.input_type ||
                t("选择定义、服务和方法后，输入 Protobuf JSON 请求。")}
            </Text>
            <Flex gap="2" wrap="wrap">
              <Button
                size="1"
                color="gray"
                variant="soft"
                disabled={state.disabled || !method || (!connectionSettings && (!method.client_streaming || halfClosed))}
                onClick={() =>
                  update({
                    message_source: JSON.stringify(
                      method!.input_template,
                      null,
                      2,
                    ),
                  })
                }
              >
                <Braces size={14} /> {t("插入模板")} </Button>
              {state.channel && method?.client_streaming && (
                <>
                  <Button
                    size="1"
                    disabled={state.disabled || session?.state !== "open" || halfClosed}
                    loading={sending}
                    onClick={() => void transmit()}
                  >
                    <Send size={14} /> {t("发送消息")} </Button>
                  <Button
                    size="1"
                    color="gray"
                    variant="soft"
                    disabled={
                      state.disabled || session?.state !== "open" || halfClosed || sending
                    }
                    onClick={() => void halfClose()}
                  >
                    <StepForward size={14} />
                    {halfClosed ? t("发送已结束") : t("结束发送")}
                  </Button>
                </>
              )}
            </Flex>
          </div>
          <Editor
            value={grpc.message_source}
            onChange={(message_source) => update({ message_source })}
            dark={state.dark}
            jsonMode
            readOnly={state.disabled || (!connectionSettings && (!method?.client_streaming || !!halfClosed || session?.state !== "open"))}
            height={state.editorHeight ?? "100%"}
            label={t("gRPC 请求消息")}
          />
          <Text size="1" color="gray" className="grpc-input-help"> {connectionSettings ? t("首次调用发送当前消息；客户端流和双向流可继续发送，结束发送后仍可接收响应。") : t("首条消息来自已保存请求。后续流消息仅用于当前调用，不会自动保存。")} </Text>
        </Tabs.Content>
        <Tabs.Content value="events">
          <GrpcEvents
            events={events}
            dropped={dropped}
            dark={state.dark}
            sessionId={session?.id}
          />
        </Tabs.Content>
        <Tabs.Content value="definition">
          {method ? (
            <Editor
              value={JSON.stringify(
                {
                  method,
                  input: state.schema?.messages?.find(
                    (item) => item.name === method.input_type,
                  ),
                  output: state.schema?.messages?.find(
                    (item) => item.name === method.output_type,
                  ),
                  enums: state.schema?.enums,
                },
                null,
                2,
              )}
              dark={state.dark}
              jsonMode
              readOnly
              height={state.editorHeight ?? "100%"}
              label={t("gRPC 方法定义")}
            />
          ) : (
            <Text color="gray"> {t("导入 Proto 或读取 Server Reflection，然后选择服务与方法。")} </Text>
          )}
        </Tabs.Content>
      </Tabs.Root>
    </section>
  );
}
