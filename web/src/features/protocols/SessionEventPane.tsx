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
    case "socketio_event":
      return event.message.event;
    case "socketio_ack":
      return `ACK ${event.message.status}`;
    case "grpc_message":
      return event.direction === "incoming" ? "响应消息" : "请求消息";
    case "grpc_metadata":
      return event.message.phase === "headers" ? "Headers" : "Trailers";
    case "grpc_status":
      return `${event.message.code} ${event.message.name}`;
    case "state":
      return "调用状态";
    case "script_log":
      return "脚本日志";
    case "script_test":
      return "脚本断言";
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
          aria-label={`搜索 ${protocolLabel} 消息`}
          placeholder="搜索消息…"
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
          label="消息方向"
          options={[
            { value: "all", label: "全部" },
            { value: "incoming", label: "接收" },
            { value: "outgoing", label: "发送" },
            { value: "system", label: "系统" },
          ]}
        />
      </Flex>
      {dropped > 0 && (
        <Text size="1" color="gray">
          已错过或淘汰 {dropped} 条事件，当前保留最近 256 条 / 8 MiB。
        </Text>
      )}
      <div className="protocol-events grpc-events">
        <ScrollArea className="protocol-event-list" type="auto">
          <div role="list" aria-label={`${protocolLabel} 调用事件`}>
            {!filtered.length && (
              <Text size="2" color="gray">
                {events.length
                  ? "没有匹配的事件。"
                  : "调用后查看消息、Metadata 和状态。"}
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
                    <ArrowDownLeft size={14} aria-label="接收" />
                  ) : event.direction === "outgoing" ? (
                    <ArrowUpRight size={14} aria-label="发送" />
                  ) : (
                    <span aria-label="系统">·</span>
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
                picked.message.kind === "socketio_event" &&
                picked.message.ack_id &&
                picked.direction === "incoming" && (
                  <Button
                    size="1"
                    variant="soft"
                    disabled={canReply ? !canReply(picked) : false}
                    onClick={() => onReply(picked)}
                  >
                    {canReply && !canReply(picked) ? "已回复 ACK" : "回复 ACK"}
                  </Button>
                )}
              <Editor
                value={messageContent(picked)}
                jsonMode
                dark={dark}
                readOnly
                height="100%"
                label={`${protocolLabel} 事件内容`}
              />
            </>
          )}
        </div>
      </div>
    </div>
  );
}
