import { useEffect, useRef, useState } from "react";
import {
  Badge,
  Button,
  Callout,
  Checkbox,
  Dialog,
  Flex,
  Tabs,
  Text,
  TextArea,
  TextField,
} from "@radix-ui/themes";
import { Plus, Send, Square, Trash2 } from "lucide-react";
import { Editor, Field, ToolButton } from "../../shared/ui";
import { bytes } from "../../shared/model";
import type { SocketIoConfig } from "../../shared/types";
import type { ProtocolEvent } from "../protocols/types";
import SessionEventPane from "../protocols/SessionEventPane";
import { useWorkbench } from "../workbench/context";
import {
  attachmentLines,
  emitCommand,
  formatAttachmentLines,
} from "./messages";
export default function SocketIoWorkbench() {
  const state = useWorkbench();
  const latest = useRef(state);
  latest.current = state;
  const { session, events, dropped, error, busy, sending, send, close } =
    state.protocolSession;
  const config =
    state.request?.protocol?.kind === "socketio"
      ? state.request.protocol
      : null;
  const [tab, setTab] = useState("send");
  const [listener, setListener] = useState("");
  const [changingListener, setChangingListener] = useState(false);
  const [answeredAcks, setAnsweredAcks] = useState(() => new Set<string>());
  const [replyTarget, setReplyTarget] = useState<string | null>(null);
  const [replySource, setReplySource] = useState("[]");
  const [replyAttachments, setReplyAttachments] = useState("");
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  const identity = JSON.stringify([
    state.authenticated,
    state.accountId,
    state.draft?.id,
    state.request?.id,
    state.draft?.data.active_environment_id,
    session?.id,
  ]);
  const scope = useRef(identity);
  scope.current = identity;
  const replyRef = useRef(replyTarget);
  replyRef.current = replyTarget;
  useEffect(() => {
    setReplyTarget(null);
    setAnsweredAcks(new Set());
    setReplySource("[]");
    setReplyAttachments("");
    setChangingListener(false);
  }, [identity]);
  useEffect(() => {
    setTab("send");
    setListener("");
  }, [state.accountId, state.draft?.id, state.request?.id]);
  const active = !!session && ["connecting", "open"].includes(session.state);
  const open = session?.state === "open";
  function update(patch: Partial<SocketIoConfig>) {
    const current = latest.current.request?.protocol;
    if (current?.kind === "socketio")
      latest.current.updateRequest({ protocol: { ...current, ...patch } });
  }
  async function changeListener(event: string, enabled: boolean) {
    event = event.trim();
    if (!event || !config || changingListener) return;
    const origin = identity;
    setChangingListener(true);
    try {
      if (active && !(await send({ kind: "socketio_listen", event, enabled })))
        return;
      if (!mounted.current || scope.current !== origin) return;
      const current = latest.current.request?.protocol;
      if (current?.kind !== "socketio") return;
      update({
        listeners: enabled
          ? [...new Set([...current.listeners, event])]
          : current.listeners.filter((item) => item !== event),
      });
      setListener("");
    } finally {
      if (mounted.current && scope.current === origin)
        setChangingListener(false);
    }
  }
  function reply(event: ProtocolEvent) {
    if (
      sending ||
      event.message.kind !== "socketio_event" ||
      !event.message.ack_id ||
      answeredAcks.has(event.message.ack_id)
    )
      return;
    setReplyTarget(event.message.ack_id);
    setReplySource("[]");
    setReplyAttachments("");
  }
  async function sendReply() {
    if (!replyTarget || !open) return;
    const origin = identity;
    const token = replyTarget;
    const accepted = await send({
      kind: "socketio_ack",
      ack_id: token,
      arguments_source: replySource,
      attachments_base64: attachmentLines(replyAttachments),
    });
    if (accepted && mounted.current && scope.current === origin) {
      // Acceptance consumes the callback even if the user dismissed its dialog.
      setAnsweredAcks((values) => new Set([...values, token]));
      if (replyRef.current === token) setReplyTarget(null);
    }
  }
  if (!config) return null;
  return (
    <section className="socketio-workbench" aria-label="Socket.IO 客户端">
      <div className="socketio-toolbar">
        <Flex gap="3" align="end" wrap="wrap">
          <Field label="Namespace">
            <TextField.Root
              value={config.namespace}
              disabled={active}
              placeholder="/"
              onChange={(event) => update({ namespace: event.target.value })}
            />
          </Field>
          <Field label="Engine.IO 路径">
            <TextField.Root
              value={config.path}
              disabled={active}
              placeholder="/socket.io/"
              onChange={(event) => update({ path: event.target.value })}
            />
          </Field>
          <Badge color="gray">Socket.IO · WebSocket</Badge>
          {session && (
            <Text size="1" color="gray" className="mono">
              ↓ {bytes(session.received_bytes)} · ↑ {bytes(session.sent_bytes)}
            </Text>
          )}
          {(active || busy) && (
            <Button
              size="1"
              color="gray"
              variant="soft"
              onClick={() => void close()}
            >
              <Square size={14} />
              {busy ? "取消连接" : "断开"}
            </Button>
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
      <Tabs.Root value={tab} onValueChange={setTab} className="grpc-tabs">
        <Tabs.List>
          <Tabs.Trigger value="send">发送事件</Tabs.Trigger>
          <Tabs.Trigger value="events">
            事件与 ACK <span className="count">{events.length}</span>
          </Tabs.Trigger>
          <Tabs.Trigger value="listeners">监听事件</Tabs.Trigger>
          <Tabs.Trigger value="auth">连接 Auth</Tabs.Trigger>
        </Tabs.List>
        <Tabs.Content value="send">
          <Flex gap="3" align="end" wrap="wrap">
            <Field label="发送事件名称">
              <TextField.Root
                value={config.event}
                onChange={(event) => update({ event: event.target.value })}
              />
            </Field>
            <label className="checkbox-label">
              <Checkbox
                checked={config.request_ack}
                onCheckedChange={(value) =>
                  update({ request_ack: value === true })
                }
              />
              请求 ACK
            </label>
            {config.request_ack && (
              <Field label="ACK 超时（ms）">
                <TextField.Root
                  type="number"
                  min="1"
                  max="120000"
                  value={config.ack_timeout_ms}
                  onChange={(event) =>
                    update({ ack_timeout_ms: Number(event.target.value) })
                  }
                />
              </Field>
            )}
            <Button
              size="2"
              disabled={!open || !config.event}
              loading={sending}
              onClick={() => void send(emitCommand(config))}
            >
              <Send size={14} />
              发送事件
            </Button>
          </Flex>
          <Editor
            value={config.arguments_source}
            onChange={(arguments_source) => update({ arguments_source })}
            dark={state.dark}
            jsonMode
            height="100%"
            label="Socket.IO 事件参数"
          />
          <Field label="二进制附件（每行一个 Base64）">
            <TextArea
              value={formatAttachmentLines(config.attachments_base64)}
              onChange={(event) =>
                update({
                  attachments_base64: attachmentLines(event.target.value),
                })
              }
              rows={2}
            />
          </Field>
          <Text size="1" color="gray">
            参数是 JSON 数组。使用 {`{"_placeholder":true,"num":0}`}{" "}
            在任意位置引用第一个附件，后续附件依次编号。
          </Text>
        </Tabs.Content>
        <Tabs.Content value="events">
          <SessionEventPane
            protocolLabel="Socket.IO"
            events={events}
            dropped={dropped}
            dark={state.dark}
            sessionId={session?.id}
            onReply={open && !sending ? reply : undefined}
            canReply={(event) =>
              event.message.kind === "socketio_event" &&
              !!event.message.ack_id &&
              !answeredAcks.has(event.message.ack_id)
            }
          />
        </Tabs.Content>
        <Tabs.Content value="listeners">
          <Flex gap="3" align="end">
            <Field label="监听事件名称">
              <TextField.Root
                value={listener}
                onChange={(event) => setListener(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.preventDefault();
                    void changeListener(listener, true);
                  }
                }}
              />
            </Field>
            <Button
              loading={changingListener}
              disabled={!listener.trim() || sending || (active && !open)}
              onClick={() => void changeListener(listener, true)}
            >
              <Plus size={15} />
              添加监听
            </Button>
          </Flex>
          <div
            className="socketio-listeners"
            role="list"
            aria-label="监听的 Socket.IO 事件"
          >
            {config.listeners.map((event) => (
              <div key={event} role="listitem" className="socketio-listener">
                <Text className="mono" size="2">
                  {event}
                </Text>
                <ToolButton
                  label={`移除监听 ${event}`}
                  disabled={changingListener || sending || (active && !open)}
                  onClick={() => void changeListener(event, false)}
                >
                  <Trash2 size={15} />
                </ToolButton>
              </div>
            ))}
            {!config.listeners.length && (
              <Text color="gray">添加要接收的事件名称。</Text>
            )}
          </div>
        </Tabs.Content>
        <Tabs.Content value="auth">
          <Text size="2" color="gray">
            Namespace 连接时发送的 Auth
            JSON，支持所选环境变量。请求头与查询参数在上方配置。
          </Text>
          <Editor
            value={config.auth_source}
            onChange={(auth_source) => update({ auth_source })}
            dark={state.dark}
            jsonMode
            readOnly={active}
            height="100%"
            label="Socket.IO Auth JSON"
          />
        </Tabs.Content>
      </Tabs.Root>
      <Dialog.Root
        open={!!replyTarget}
        onOpenChange={(value) => {
          if (!value) setReplyTarget(null);
        }}
      >
        <Dialog.Content maxWidth="700px">
          <Dialog.Title>回复服务端 ACK</Dialog.Title>
          <Dialog.Description size="2">
            只回复当前会话中服务端请求的确认；令牌过期或已回复时，服务端会拒绝重复回复。
          </Dialog.Description>
          <Editor
            value={replySource}
            onChange={setReplySource}
            dark={state.dark}
            jsonMode
            height="240px"
            readOnly={sending}
            label="ACK 回复参数"
          />
          <Field label="ACK 二进制附件（每行一个 Base64）">
            <TextArea
              value={replyAttachments}
              disabled={sending}
              onChange={(event) => setReplyAttachments(event.target.value)}
            />
          </Field>
          <Flex gap="3" justify="end" mt="4">
            <Dialog.Close>
              <Button variant="soft" color="gray">
                取消
              </Button>
            </Dialog.Close>
            <Button
              loading={sending}
              disabled={!open}
              onClick={() => void sendReply()}
            >
              回复 ACK
            </Button>
          </Flex>
        </Dialog.Content>
      </Dialog.Root>
    </section>
  );
}
