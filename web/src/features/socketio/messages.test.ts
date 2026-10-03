import { describe, expect, it } from "vitest";
import {
  attachmentLines,
  emitCommand,
  formatAttachmentLines,
} from "./messages";
import type { SocketIoConfig } from "../../shared/types";
const draft: SocketIoConfig = {
  kind: "socketio",
  namespace: "/",
  path: "/socket.io/",
  auth_source: "{}",
  listeners: ["message"],
  event: "echo",
  arguments_source: '[{"nested":{"_placeholder":true,"num":0}},42]',
  attachments_base64: ["AP8="],
  request_ack: true,
  ack_timeout_ms: 5000,
};
describe("Socket.IO command drafts", () => {
  it("preserves positional JSON and binary attachments while assigning fresh ACK correlations", () => {
    const first = emitCommand(draft);
    const second = emitCommand(draft);
    expect(first).toMatchObject({
      kind: "socketio_emit",
      event: "echo",
      arguments_source: draft.arguments_source,
      attachments_base64: ["AP8="],
      ack_timeout_ms: 5000,
    });
    if (first.kind !== "socketio_emit" || second.kind !== "socketio_emit")
      throw new Error("wrong command");
    expect(first.ack_id).not.toBeNull();
    expect(first.ack_id).not.toBe(second.ack_id);
    expect(emitCommand({ ...draft, request_ack: false })).toMatchObject({
      ack_id: null,
    });
  });
  it("keeps malformed argument drafts intact for actual Rust validation", () => {
    expect(
      emitCommand({ ...draft, arguments_source: "[invalid draft" }),
    ).toMatchObject({ arguments_source: "[invalid draft" });
  });
  it("retains explicit empty binary attachments and positional order", () => {
    expect(attachmentLines('AP8=\r\n\n""\nAQI=')).toEqual(["AP8=", "", "AQI="]);
    expect(
      attachmentLines(formatAttachmentLines(["AP8=", "", "AQI="])),
    ).toEqual(["AP8=", "", "AQI="]);
  });
});
