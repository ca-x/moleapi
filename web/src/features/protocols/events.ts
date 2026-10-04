import { t } from "../../shared/i18n";
import { base64, hex } from "@scure/base";
import type { EventBatch, ProtocolEvent } from "./types";
const MAX_EVENTS = 256;
const MAX_BYTES = 8 * 1024 * 1024;
/** Match server retention bounds and deduplicate retries by the stable cursor. */
export function appendEvents(
  previous: ProtocolEvent[],
  batch: EventBatch,
): { events: ProtocolEvent[]; dropped: number } {
  const merged = new Map(previous.map((event) => [event.cursor, event]));
  for (const event of batch.events) merged.set(event.cursor, event);
  const ordered = [...merged.values()].sort((a, b) => a.cursor - b.cursor);
  const kept: ProtocolEvent[] = [];
  let size = 0;
  const encoder = new TextEncoder();
  for (let index = ordered.length - 1; index >= 0; index--) {
    const event = ordered[index];
    const bytes = encoder.encode(JSON.stringify(event)).byteLength;
    if (kept.length >= MAX_EVENTS || size + bytes > MAX_BYTES) break;
    kept.push(event);
    size += bytes;
  }
  return {
    events: kept.reverse(),
    dropped: ordered.length - kept.length + batch.dropped_count,
  };
}
export function messageContent(event: ProtocolEvent): string {
  const message = event.message;
  switch (message.kind) {
    case "tcp_data": {
      if(message.redacted)return t("私密载荷已隐藏 · {{value0}} 字节", { value0: message.bytes });
      try {return [message.text!==null ? `UTF8:\n${message.text}` : t("UTF8: 非文本数据"),`Hex:\n${hex.encode(base64.decode(message.base64))}`,`Base64:\n${message.base64}`].join("\n\n");}
      catch {return JSON.stringify(message,null,2);}
    }
    case "tcp_half_closed": return t("TCP 发送端已半关闭，继续接收");
    case "a2a_ready":
    case "a2a_result":
    case "a2a_stream":
    case "a2a_error":
    case "a2a_finished":
      return JSON.stringify(message, null, 2);
    case "mcp_initialized":
    case "mcp_capabilities":
    case "mcp_result":
    case "mcp_error":
    case "mcp_notification":
    case "mcp_callback":
      return JSON.stringify(message, null, 2);
    case "mqtt_message":
    case "mqtt_status":
      return JSON.stringify(message, null, 2);
    case "socketio_event":
    case "socketio_ack":
      return JSON.stringify(message, null, 2);
    case "grpc_message":
      return JSON.stringify(message.message, null, 2);
    case "grpc_metadata":
      return JSON.stringify(
        { phase: message.phase, metadata: message.metadata },
        null,
        2,
      );
    case "grpc_status":
      return JSON.stringify(message, null, 2);
    case "graphql_next":
    case "graphql_error":
    case "graphql_complete":
      return JSON.stringify(message.payload, null, 2);
    case "sse":
      return message.data;
    case "text":
      return message.text;
    case "binary":
    case "ping":
    case "pong":
      return message.base64;
    case "close":
      return `${message.code ?? ""} ${message.reason}`.trim();
    case "state":
      return message.reason || message.state;
    case "script_log":
      return message.message;
    case "script_test":
      return `${message.test.name}: ${message.test.passed ? t("通过") : t("失败")} · ${message.test.actual}`;
  }
}
