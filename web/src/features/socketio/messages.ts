import { id } from "../../shared/model";
import type { SocketIoConfig } from "../../shared/types";
import type { SendMessage } from "../protocols/types";
export const attachmentLines = (text: string) =>
  text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => (line === '""' ? "" : line));
export function emitCommand(config: SocketIoConfig): SendMessage {
  return {
    kind: "socketio_emit",
    event: config.event,
    arguments_source: config.arguments_source,
    attachments_base64: config.attachments_base64,
    ack_timeout_ms: config.ack_timeout_ms,
    ack_id: config.request_ack ? id() : null,
  };
}

export const formatAttachmentLines = (values: string[]) =>
  values.map((value) => value || '""').join("\n");
