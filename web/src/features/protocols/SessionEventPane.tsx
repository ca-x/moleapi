import { t, useLanguage } from "../../shared/i18n";
import { useEffect, useState } from "react";
import {
  Badge,
  Button,
  Flex,
  ScrollArea,
  Text,
  TextField,
} from "@radix-ui/themes";
import { ArrowDownLeft, ArrowUpRight, Search } from "lucide-react";
import { Choice, Editor } from "../../shared/ui";
import { messageContent } from "./events";
import type { ProtocolEvent } from "./types";
const eventLabel = (event: ProtocolEvent) => {
  switch (event.message.kind) {
    case "a2a_ready": return t("A2A 就绪");
    case "a2a_result": return t("{{method}} 结果", { method: event.message.method });
    case "a2a_stream": return t("{{method}} 事件", { method: event.message.method });
    case "a2a_error": return t("{{method}} 错误", { method: event.message.method });
    case "a2a_finished": return t("{{method}} 完成", { method: event.message.method });
    case "mcp_initialized": return t("MCP 初始化");
    case "mcp_capabilities": return t("MCP 能力列表");
    case "mcp_result": return t("{{method}} 结果", { method: event.message.method });
    case "mcp_error": return t("{{method}} 错误", { method: event.message.method });
    case "mcp_notification": return event.message.method;
    case "mcp_callback": return t("{{method}} 回调", { method: event.message.method });
    case "mqtt_message":
      return event.message.topic;
    case "mqtt_status":
      return event.message.operation + " " + event.message.status;
    case "socketio_event":
      return event.message.event;
    case "socketio_ack":
      return `ACK ${event.message.status}`;
    case "grpc_message":
      return event.direction === "incoming" ? t("响应消息") : t("请求消息");
    case "grpc_metadata":
      return event.message.phase === "headers" ? "Headers" : "Trailers";
    case "grpc_status":
      return `${event.message.code} ${event.message.name}`;
    case "state":
      return t("调用状态");
    case "script_log":
      return t("脚本日志");
    case "script_test":
      return t("脚本断言");
    default:
      return event.message.kind;
  }
};
export default function SessionEventPane({
  events,
  dropped,
  dark,
  sessionId,
  protocolLabel = "gRPC",
  onReply,
  canReply,
}: {
  events: ProtocolEvent[];
  dropped: number;
  dark: boolean;
  sessionId?: string;
  protocolLabel?: string;
  onReply?: (event: ProtocolEvent) => void;
  canReply?: (event: ProtocolEvent) => boolean;
}) {
  useLanguage();
  const [selected, setSelected] = useState<number | null>(null);
  const [search, setSearch] = useState("");
  const [direction, setDirection] = useState("all");
  useEffect(() => {
    setSelected(null);
    setSearch("");
    setDirection("all");
  }, [sessionId]);
  const filtered = events.filter(
    (event) =>
      (direction === "all" || event.direction === direction) &&
      `${eventLabel(event)} ${messageContent(event)}`
        .toLowerCase()
        .includes(search.toLowerCase()),
  );
  const picked =
    filtered.find((event) => event.cursor === selected) || filtered.at(-1);
  return (
    <div className="grpc-event-pane">
      <Flex gap="3" className="grpc-event-controls">
        <TextField.Root
          aria-label={t("搜索 {{value0}} 消息", { value0: protocolLabel })}
          placeholder={t("搜索消息…")}
          value={search}
          onChange={(event) => setSearch(event.target.value)}
        >
          <TextField.Slot>
            <Search size={14} />
          </TextField.Slot>
        </TextField.Root>
        <Choice
          value={direction}
          onChange={setDirection}
          label={t("消息方向")}
          options={[
            { value: "all", label: t("全部") },
            { value: "incoming", label: t("接收") },
            { value: "outgoing", label: t("发送") },
            { value: "system", label: t("系统") },
          ]}
        />
      </Flex>
      {dropped > 0 && (
        <Text size="1" color="gray"> {t("已错过或淘汰 {{count}} 条事件，当前保留最近 256 条 / 8 MiB。", { count: dropped })} </Text>
      )}
      <div className="protocol-events grpc-events">
        <ScrollArea className="protocol-event-list" type="auto">
          <div role="list" aria-label={t("{{value0}} 调用事件", { value0: protocolLabel })}>
            {!filtered.length && (
              <Text size="2" color="gray">
                {events.length
                  ? t("没有匹配的事件。")
                  : t("调用后查看消息、Metadata 和状态。")}
              </Text>
            )}
            {filtered.map((event) => (
              <div role="listitem" key={event.cursor}>
                <button
                  className={`protocol-event-row ${picked?.cursor === event.cursor ? "selected" : ""}`}
                  aria-pressed={picked?.cursor === event.cursor}
                  onClick={() => setSelected(event.cursor)}
                >
                  <span className="mono">{event.cursor}</span>
                  {event.direction === "incoming" ? (
                    <ArrowDownLeft size={14} aria-label={t("接收")} />
                  ) : event.direction === "outgoing" ? (
                    <ArrowUpRight size={14} aria-label={t("发送")} />
                  ) : (
                    <span aria-label={t("系统")}>·</span>
                  )}
                  <span className="protocol-event-preview">
                    <span>{eventLabel(event)}</span>
                    <span className="mono">
                      {messageContent(event).slice(0, 120)}
                    </span>
                  </span>
                </button>
              </div>
            ))}
          </div>
        </ScrollArea>
        <div className="protocol-event-detail">
          {picked && (
            <>
              <Flex gap="3" align="center" mb="2">
                <Badge color="gray">{eventLabel(picked)}</Badge>
                <Text size="1" color="gray" className="mono">
                  #{picked.cursor} · {picked.received_at}
                </Text>
              </Flex>
              {onReply &&
                ((picked.message.kind === "socketio_event" && picked.message.ack_id) || picked.message.kind === "mcp_callback") &&
                picked.direction === "incoming" && (
                  <Button
                    size="1"
                    variant="soft"
                    disabled={canReply ? !canReply(picked) : false}
                    onClick={() => onReply(picked)}
                  >
                    {picked.message.kind === "mcp_callback" ? (canReply && !canReply(picked) ? t("回调已结束") : t("回复回调")) : (canReply && !canReply(picked) ? t("已回复 ACK") : t("回复 ACK"))}
                  </Button>
                )}
              <Editor
                value={messageContent(picked)}
                jsonMode
                dark={dark}
                readOnly
                height="100%"
                label={t("{{value0}} 事件内容", { value0: protocolLabel })}
              />
            </>
          )}
        </div>
      </div>
    </div>
  );
}
