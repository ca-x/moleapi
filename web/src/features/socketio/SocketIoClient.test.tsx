// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { Theme } from "@radix-ui/themes";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { setLanguage } from "../../shared/i18n";
import type { ProtocolEvent, ProtocolSession } from "../protocols/types";
import SocketIoClient, { type SocketIoChannel } from "./SocketIoClient";
import { socketIoConfig } from "./model";
const callbacks = vi.hoisted(() => ({ reply: undefined as undefined | ((event: ProtocolEvent) => void) }));
vi.mock("../../shared/ui", () => ({ Editor: ({ value, label, onChange }: { value: string; label: string; onChange?: (value: string) => void }) => <textarea aria-label={label} value={value} onChange={(e) => onChange?.(e.target.value)} />, Field: ({ label, children }: { label: string; children: React.ReactNode }) => <label>{label}{children}</label>, ToolButton: ({ label, disabled, onClick }: { label: string; disabled: boolean; onClick: () => void }) => <button aria-label={label} disabled={disabled} onClick={onClick} /> }));
vi.mock("../protocols/SessionEventPane", () => ({ default: ({ onReply }: { onReply?: (event: ProtocolEvent) => void }) => { callbacks.reply = onReply; return <div>events</div>; } }));
const session: ProtocolSession = { id: "s", workspace_id: "w", request_id: "r", protocol: "socketio", state: "open", url: "ws://fixture.test", created_at: "now", updated_at: "now", reason: null, received_bytes: 0, sent_bytes: 0, event_count: 0, handshake: null };
const question: ProtocolEvent = { cursor: 1, received_at: "now", direction: "incoming", message: { kind: "socketio_event", event: "question", arguments: [], attachments_base64: [], ack_id: "callback" } };
let valid: boolean;
let channel: SocketIoChannel;
const change = vi.fn();
function view(scope = "a", channelOverride = channel) { return <Theme><SocketIoClient scope={scope} config={socketIoConfig()} onChange={change} isCurrent={() => valid} dark={false} channel={channelOverride} /></Theme>; }
beforeEach(async () => { await setLanguage("zh-CN"); valid = true; change.mockReset(); channel = { session, events: [question], dropped: 0, busy: false, sending: false, error: "", send: vi.fn().mockResolvedValue(true), close: vi.fn().mockResolvedValue(undefined) }; });
afterEach(cleanup);
it("clears listener busy state after admission expires without applying the late listener result", async () => {
  let finish!: (value: boolean) => void; vi.mocked(channel.send).mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
  const { rerender } = render(view());
  fireEvent.mouseDown(screen.getByRole("tab", { name: /监听事件/ }), { button: 0 });
  fireEvent.change(screen.getByLabelText("监听事件名称"), { target: { value: "question" } });
  fireEvent.click(screen.getByRole("button", { name: "添加监听" }));
  valid = false; await act(async () => finish(true));
  expect(change).not.toHaveBeenCalled(); valid = true; rerender(view());
  expect((screen.getByRole("button", { name: "添加监听" }) as HTMLButtonElement).disabled).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "添加监听" }));
  expect(channel.send).toHaveBeenCalledTimes(2);
  await act(async () => finish(false));
});
it("rejects a retained ACK callback after the same session becomes terminal", () => {
  const { rerender } = render(view());
  fireEvent.mouseDown(screen.getByRole("tab", { name: /事件与 ACK/ }), { button: 0 });
  const retained = callbacks.reply!;
  rerender(view("a", { ...channel, session: { ...session, state: "closed" } }));
  act(() => retained(question));
  expect(screen.queryByRole("dialog")).toBeNull(); expect(channel.send).not.toHaveBeenCalled();
});
it("rejects retained reply callbacks after a scope/session switch", () => {
  const { rerender } = render(view());
  fireEvent.mouseDown(screen.getByRole("tab", { name: /事件与 ACK/ }), { button: 0 }); const retained = callbacks.reply!;
  rerender(view("other", { ...channel, session: { ...session, id: "new" } }));
  act(() => retained(question)); expect(screen.queryByRole("dialog")).toBeNull(); expect(channel.send).not.toHaveBeenCalled();
});
it("does not send an ACK reply after authoritative admission changes before confirmation", () => {
  render(view()); fireEvent.mouseDown(screen.getByRole("tab", { name: /事件与 ACK/ }), { button: 0 });
  act(() => callbacks.reply!(question)); valid = false;
  fireEvent.click(screen.getByRole("button", { name: "回复 ACK" })); expect(channel.send).not.toHaveBeenCalled();
});
