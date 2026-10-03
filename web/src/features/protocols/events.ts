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
      return `${message.test.name}: ${message.test.passed ? "通过" : "失败"} · ${message.test.actual}`;
  }
}
