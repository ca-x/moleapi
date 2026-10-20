// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { Theme } from "@radix-ui/themes";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { setLanguage } from "../../shared/i18n";
import { newRequest } from "../../shared/model";
import type { ProtocolSession } from "../protocols/types";
import GrpcClient, { type GrpcChannel } from "./GrpcClient";
vi.mock("../../shared/ui", () => ({ Editor: ({ label, value, readOnly, onChange }: { label: string; value: string; readOnly?: boolean; onChange?: (value: string) => void }) => <textarea aria-label={label} value={value} readOnly={readOnly} onChange={(e) => onChange?.(e.target.value)} />, Choice: () => null }));
vi.mock("./GrpcEvents", () => ({ default: () => null }));
const session: ProtocolSession = { id: "s", workspace_id: "w", request_id: "r", protocol: "grpc", state: "open", url: "http://fixture.test", created_at: "now", updated_at: "now", reason: null, received_bytes: 0, sent_bytes: 0, event_count: 0, handshake: null };
function props(client: boolean, server: boolean) {
  const channel: GrpcChannel = { session, events: [], dropped: 0, busy: false, sending: false, error: "", send: vi.fn().mockResolvedValue(true), close: vi.fn().mockResolvedValue(undefined) };
  return { scope: "member", request: { ...newRequest(), protocol: { kind: "grpc" as const, service: "demo.EchoService", method: "Call", message_source: '{"text":"saved"}' } }, specifications: [], schema: { services: [{ name: "demo.EchoService", methods: [{ name: "Call", full_name: "demo.EchoService.Call", input_type: "demo.Echo", output_type: "demo.Echo", client_streaming: client, server_streaming: server, input_template: { text: "", count: "0" } }] }] }, channel, dark: false, connectionSettings: false, onChange: vi.fn(), isCurrent: () => true };
}
beforeEach(async () => { await setLanguage("zh-CN"); });
afterEach(cleanup);
it.each([[false, false], [false, true], [true, false], [true, true]])("exposes extra stream controls only for client-streaming modes %s/%s", (client, server) => {
  const state = props(client, server); render(<Theme><GrpcClient {...state} /></Theme>);
  expect(!!screen.queryByRole("button", { name: "发送消息" })).toBe(client);
  expect(!!screen.queryByRole("button", { name: "结束发送" })).toBe(client);
  expect((screen.getByLabelText("gRPC 请求消息") as HTMLTextAreaElement).readOnly).toBe(!client);
  expect(state.channel.send).not.toHaveBeenCalled();
});
it("withholds stream commands after current cache/credential admission changes before click", () => {
  const state = props(true, true); let valid = true; state.isCurrent = () => valid;
  render(<Theme><GrpcClient {...state} /></Theme>); valid = false;
  fireEvent.click(screen.getByRole("button", { name: "发送消息" })); fireEvent.click(screen.getByRole("button", { name: "结束发送" }));
  expect(state.channel.send).not.toHaveBeenCalled();
});
it("inserts the descriptor template as an edit without sending it automatically", () => {
  const state = props(true, false); render(<Theme><GrpcClient {...state} /></Theme>);
  fireEvent.click(screen.getByRole("button", { name: "插入模板" }));
  expect(state.onChange).toHaveBeenCalledWith({ protocol: { ...state.request.protocol, message_source: JSON.stringify({ text: "", count: "0" }, null, 2) } });
  expect(state.channel.send).not.toHaveBeenCalled();
});
