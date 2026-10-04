import { t, useLanguage } from "../../shared/i18n";
import { useState } from "react";
import {
  Badge,
  Button,
  Callout,
  Flex,
  Heading,
  ScrollArea,
  Table,
  Tabs,
  Text,
} from "@radix-ui/themes";
import { ArrowDownLeft, ArrowUpRight, Plug, Unplug, Send } from "lucide-react";
import { Editor, Choice } from "../../shared/ui";
import { bytes } from "../../shared/model";
import { useWorkbench } from "../workbench/context";
import { messageContent } from "./events";
export default function ProtocolPane() {
  useLanguage();
  const { protocolSession, dark } = useWorkbench();
  const { session, events, dropped, error, busy, sending, close, send } =
    protocolSession;
  const [selected, setSelected] = useState<number | null>(null);
  const [mode, setMode] = useState<"text" | "binary" | "ping">("text");
  const [payload, setPayload] = useState("");
  const picked =
    events.find((event) => event.cursor === selected) || events.at(-1);
  const open = session?.state === "open";
  const websocket = session?.protocol === "websocket";
  const submit = () =>
    void send(
      mode === "text"
        ? { kind: mode, text: payload }
        : { kind: mode, base64: payload },
    );
  return (
    <section
      className="response-pane response-pane-fill protocol-pane"
      aria-label={t("协议会话面板")}
    >
      <div className="response-toolbar">
        <Flex align="center" gap="3">
          <Text size="2" weight="medium"> {t("实时会话")} </Text>
          {session && (
            <>
              <Badge
                color={
                  session.state === "open"
                    ? "green"
                    : session.state === "error"
                      ? "red"
                      : session.state === "connecting"
                        ? "amber"
                        : "gray"
                }
              >
                {
                  (
                    {
                      connecting: t("连接中"),
                      open: t("已连接"),
                      closed: t("已关闭"),
                      error: t("连接失败"),
                    } as const
                  )[session.state]
                }
              </Badge>
              <Text size="1" className="mono" color="gray">
                ↓ {bytes(session.received_bytes)} · ↑{" "}
                {bytes(session.sent_bytes)}
              </Text>
            </>
          )}
        </Flex>
        {session && (open || session.state === "connecting") && (
          <Button
            size="1"
            variant="soft"
            color="gray"
            onClick={() => void close()}
          >
            <Unplug size={14} /> {t("断开")} </Button>
        )}
      </div>
      {error && (
        <Callout.Root color="red" role="alert">
          <Callout.Text>{error}</Callout.Text>
        </Callout.Root>
      )}
      {session?.reason && (
        <Text size="1" className="protocol-reason wrap-anywhere">
          {session.reason}
        </Text>
      )}
      {!session && (
        <div className="response-empty">
          <Plug size={30} strokeWidth={1.3} />
          <Heading size="3">
            {busy ? t("正在创建会话…") : t("连接后查看实时消息")}
          </Heading>
          <Text size="2" color="gray"> {t("SSE 接收事件，WebSocket 支持双向消息。")} </Text>
        </div>
      )}
      {session && (
        <Tabs.Root defaultValue="events">
          <Tabs.List>
            <Tabs.Trigger value="events"> {t("消息")} <span className="count">{events.length}</span>
            </Tabs.Trigger>
            <Tabs.Trigger value="handshake">{t("握手")}</Tabs.Trigger>
            {websocket && <Tabs.Trigger value="send">{t("发送消息")}</Tabs.Trigger>}
          </Tabs.List>
          <Tabs.Content value="events">
            {dropped > 0 && (
              <Text size="1" color="gray" className="protocol-gap"> {t("已淘汰或错过 {{count}} 条消息，仅保留最近 256 条 / 8 MiB。", { count: dropped })} </Text>
            )}
            <div className="protocol-events">
              <ScrollArea className="protocol-event-list" type="auto">
                <div role="list" aria-label={t("会话消息")}>
                  {events.length === 0 ? (
                    <Text as="p" color="gray" size="2"> {t("等待消息…")} </Text>
                  ) : (
                    events.map((event) => (
                      <div role="listitem" key={event.cursor}>
                        <button
                          className={`protocol-event-row ${picked?.cursor === event.cursor ? "selected" : ""}`}
                          onClick={() => setSelected(event.cursor)}
                          aria-pressed={picked?.cursor === event.cursor}
                        >
                          <span className="mono">{event.cursor}</span>
                          {event.direction === "incoming" ? (
                            <ArrowDownLeft size={14} aria-label={t("接收")} />
                          ) : event.direction === "outgoing" ? (
                            <ArrowUpRight size={14} aria-label={t("发送")} />
                          ) : (
                            <span aria-label={t("系统事件")}>·</span>
                          )}
                          <span className="protocol-event-preview">
                            <span>
                              {event.message.kind === "sse"
                                ? event.message.event || "message"
                                : event.message.kind}
                            </span>
                            <span className="mono">
                              {messageContent(event).slice(0, 120)}
                            </span>
                          </span>
                        </button>
                      </div>
                    ))
                  )}
                </div>
              </ScrollArea>
              <div className="protocol-event-detail">
                {picked && (
                  <>
                    <Flex gap="3" wrap="wrap" mb="2">
                      <Text size="1" color="gray" className="mono">
                        #{picked.cursor} · {picked.received_at}
                      </Text>
                      {picked.message.kind === "sse" && (
                        <Text size="1" color="gray">
                          ID {picked.message.id || "—"} · retry{" "}
                          {picked.message.retry ?? "—"}
                        </Text>
                      )}
                    </Flex>
                    <Editor
                      value={messageContent(picked)}
                      readOnly
                      dark={dark}
                      height="100%"
                      label={t("会话消息内容")}
                    />
                  </>
                )}
              </div>
            </div>
          </Tabs.Content>
          <Tabs.Content value="handshake">
            <Text as="p" size="1" color="gray" className="wrap-anywhere">
              {session.url}
            </Text>
            <Text as="p" size="2" mt="3"> {t("状态")} {session.handshake?.status ?? t("等待握手")}
            </Text>
            <Table.Root>
              <Table.Header>
                <Table.Row>
                  <Table.ColumnHeaderCell>{t("名称")}</Table.ColumnHeaderCell>
                  <Table.ColumnHeaderCell>{t("值")}</Table.ColumnHeaderCell>
                </Table.Row>
              </Table.Header>
              <Table.Body>
                {session.handshake?.headers.map((header, index) => (
                  <Table.Row key={index}>
                    <Table.Cell className="mono">{header.key}</Table.Cell>
                    <Table.Cell className="mono wrap-anywhere">
                      {header.value}
                    </Table.Cell>
                  </Table.Row>
                ))}
              </Table.Body>
            </Table.Root>
          </Tabs.Content>
          {websocket && (
            <Tabs.Content value="send">
              <Flex align="center" justify="between" gap="3" mb="3">
                <Choice
                  value={mode}
                  onChange={setMode}
                  label={t("消息类型")}
                  options={[
                    { value: "text", label: t("文本") },
                    { value: "binary", label: t("二进制 / Base64") },
                    { value: "ping", label: "Ping / Base64" },
                  ]}
                />
                <Button
                  size="2"
                  onClick={submit}
                  disabled={!open || sending}
                  loading={sending}
                >
                  <Send size={14} /> {t("发送消息")} </Button>
              </Flex>
              <div
                onKeyDown={(event) => {
                  if (
                    (event.ctrlKey || event.metaKey) &&
                    event.key === "Enter"
                  ) {
                    event.preventDefault();
                    event.stopPropagation();
                    submit();
                  }
                }}
              >
                <Editor
                  value={payload}
                  onChange={setPayload}
                  dark={dark}
                  label={
                    mode === "text"
                      ? t("WebSocket 文本消息")
                      : t("WebSocket Base64 消息")
                  }
                  height="160px"
                />
              </div>
              <Text size="1" color="gray">
                {mode === "text"
                  ? t("文本按 UTF-8 发送。")
                  : t("输入 Base64 编码内容；Ping 解码后最多 125 字节。")}{" "} {t("Ctrl / ⌘ + Enter 发送。")} </Text>
            </Tabs.Content>
          )}
        </Tabs.Root>
      )}
    </section>
  );
}
