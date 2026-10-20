// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { Theme } from "@radix-ui/themes";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { setLanguage } from "../../shared/i18n";
import { newRequest } from "../../shared/model";
import type { RequestSpec } from "../../shared/types";
import type { ProtocolSession } from "../protocols/types";
import MqttClient, { type MqttChannel } from "./MqttClient";
import MqttSubscriptions from "./MqttSubscriptions";
import MqttWorkbench from "./MqttWorkbench";
import { mqttConfig } from "./model";
const owned = vi.hoisted(() => ({ state: {} as unknown }));
vi.mock("../workbench/context", () => ({ useWorkbench: () => owned.state }));
const callbacks = vi.hoisted(() => ({ editor: undefined as undefined | ((value: string) => void), saved: undefined as undefined | ((value: string) => void) }));
vi.mock("../../shared/ui", () => ({
  Editor: ({ value, label, onChange, readOnly }: { value: string; label: string; onChange?: (value: string) => void; readOnly?: boolean }) => { callbacks.editor = onChange; return <textarea aria-label={label} value={value} readOnly={readOnly} onChange={(e) => onChange?.(e.target.value)} />; },
  Field: ({ label, children }: { label: string; children: React.ReactNode }) => <label>{label}{children}</label>,
  Choice: ({ label, value, options, onChange, disabled }: { label: string; value: string; options: { value: string; label: string }[]; onChange: (value: string) => void; disabled?: boolean }) => { if (label === "保存的 MQTT 消息") callbacks.saved = onChange; return <label>{label}<select value={value} disabled={disabled} onChange={(e) => onChange(e.target.value)}>{options.map((o) => <option value={o.value} key={o.value}>{o.label}</option>)}</select></label>; },
  ToolButton: ({ label, children, onClick, disabled }: { label: string; children: React.ReactNode; onClick: () => void; disabled?: boolean }) => <button aria-label={label} onClick={onClick} disabled={disabled}>{children}</button>,
}));
beforeEach(async () => { await setLanguage("zh-CN"); });
afterEach(cleanup);
function request(): RequestSpec { const value = newRequest(); const config = mqttConfig(); config.message.topic = "saved/topic"; config.message.payload_source = "saved payload"; value.protocol = config; return value; }
function channel(): MqttChannel { return { session: { id: "mqtt-a", protocol: "mqtt", state: "open", received_bytes: 0, sent_bytes: 0 } as ProtocolSession, events: [], dropped: 0, error: "", busy: false, sending: false, send: vi.fn().mockResolvedValue(true), close: vi.fn().mockResolvedValue(undefined) }; }
it("source editor stages named messages without a live channel or network command", () => {
  const onChange = vi.fn(); render(<Theme><MqttClient scope="source" request={request()} dark={false} isCurrent={() => true} onChange={onChange} /></Theme>);
  expect(screen.queryByRole("button", { name: "发布" })).toBeNull();
  fireEvent.change(screen.getByLabelText("消息名称"), { target: { value: "Canonical template" } });
  fireEvent.click(screen.getByRole("button", { name: "保存新消息" }));
  expect(onChange).toHaveBeenCalledWith({ protocol: expect.objectContaining({ saved_messages: [expect.objectContaining({ name: "Canonical template", message: expect.objectContaining({ payload_source: "saved payload" }) })] }) });
});
it("runtime hides connection and named resource mutation, sends only on explicit publish", async () => {
  const active = channel(); render(<Theme><MqttClient scope="s" request={request()} dark={false} isCurrent={() => true} onChange={vi.fn()} channel={active} connectionSettings={false} savedMessagesEditable={false} /></Theme>);
  expect(active.send).not.toHaveBeenCalled();
  expect(screen.queryByRole("tab", { name: /连接设置/ })).toBeNull();
  expect(screen.queryByRole("button", { name: "保存新消息" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "发布" }));
  expect(active.send).toHaveBeenCalledWith({ kind: "mqtt_publish", message: expect.objectContaining({ topic: "saved/topic", payload_source: "saved payload" }) });
});
it("retained editor callbacks cannot edit a changed scope or disposed client", () => {
  const onChange = vi.fn(); const props = { request: request(), dark: false, isCurrent: () => true, onChange };
  const { rerender, unmount } = render(<Theme><MqttClient {...props} scope="first" /></Theme>);
  const retained = callbacks.editor!;
  rerender(<Theme><MqttClient {...props} scope="second" /></Theme>);
  act(() => retained("late old source")); expect(onChange).not.toHaveBeenCalled();
  const disposed = callbacks.editor!; unmount(); act(() => disposed("disposed")); expect(onChange).not.toHaveBeenCalled();
});
it.each(["scope", "admission", "saving"])("withholds pending subscription edits after %s changes", async (boundary) => {
  let finish!: (result: boolean) => void;
  const active = channel(); active.send = vi.fn(() => new Promise<boolean>((resolve) => { finish = resolve; }));
  const update = vi.fn(); let allowed = true;
  const config = mqttConfig(); const props = { value: config, update, channel: active, isCurrent: () => allowed };
  const { rerender } = render(<Theme><MqttSubscriptions {...props} identity="first" /></Theme>);
  fireEvent.change(screen.getByLabelText("Topic Filter"), { target: { value: "pending/#" } });
  fireEvent.click(screen.getByRole("button", { name: "添加订阅" }));
  expect(active.send).toHaveBeenCalledWith({ kind: "mqtt_subscribe", subscription: expect.objectContaining({ filter: "pending/#" }) });
  if (boundary === "admission") allowed = false;
  rerender(<Theme><MqttSubscriptions {...props} identity={boundary === "scope" ? "second" : "first"} disabled={boundary === "saving"} /></Theme>);
  await act(async () => finish(true)); expect(update).not.toHaveBeenCalled();
});
it("failed subscription admission clears pending and allows a deliberate retry", async () => {
  const active = channel(); active.send = vi.fn().mockResolvedValueOnce(false).mockResolvedValueOnce(true);
  const update = vi.fn(); render(<Theme><MqttSubscriptions value={mqttConfig()} update={update} identity="same" channel={active} isCurrent={() => true} /></Theme>);
  fireEvent.change(screen.getByLabelText("Topic Filter"), { target: { value: "retry/#" } });
  await act(async () => fireEvent.click(screen.getByRole("button", { name: "添加订阅" })));
  expect(update).not.toHaveBeenCalled();
  await act(async () => fireEvent.click(screen.getByRole("button", { name: "添加订阅" })));
  expect(update).toHaveBeenCalledWith({ subscriptions: [expect.objectContaining({ filter: "retry/#" })] });
});
it("abort requires confirmation and rejects authorization lost while the dialog is open", async () => {
  const active = channel(); let allowed = true;
  render(<Theme><MqttClient scope="s" request={request()} dark={false} isCurrent={() => allowed} onChange={vi.fn()} channel={active} /></Theme>);
  fireEvent.click(screen.getByRole("button", { name: "中止连接" })); expect(active.send).not.toHaveBeenCalled();
  const dialog = await screen.findByRole("alertdialog"); allowed = false;
  await act(async () => fireEvent.click(within(dialog).getByRole("button", { name: "中止连接" })));
  expect(active.send).not.toHaveBeenCalled();
});
it("an abort confirmation cannot target a new session", async () => {
  const active = channel(); const props = { request: request(), dark: false, isCurrent: () => true, onChange: vi.fn() };
  const { rerender } = render(<Theme><MqttClient {...props} scope="same" channel={active} /></Theme>);
  fireEvent.click(screen.getByRole("button", { name: "中止连接" })); await screen.findByRole("alertdialog");
  const next = { ...active, session: { ...active.session!, id: "mqtt-b" } };
  rerender(<Theme><MqttClient {...props} scope="same" channel={next} /></Theme>);
  expect(screen.queryByRole("alertdialog")).toBeNull(); expect(active.send).not.toHaveBeenCalled();
});
it("MQTT3 publish rejects preserved MQTT5 properties and current admission loss", () => {
  const active = channel(), value = request(); if (value.protocol?.kind !== "mqtt") throw new Error();
  value.protocol.version = "3.1.1"; value.protocol.message.properties.content_type = "application/json";
  const { rerender } = render(<Theme><MqttClient scope="s" request={value} dark={false} isCurrent={() => true} onChange={vi.fn()} channel={active} /></Theme>);
  expect((screen.getByRole("button", { name: "发布" }) as HTMLButtonElement).disabled).toBe(true);
  rerender(<Theme><MqttClient scope="s" request={request()} dark={false} isCurrent={() => false} onChange={vi.fn()} channel={active} /></Theme>);
  fireEvent.click(screen.getByRole("button", { name: "发布" })); expect(active.send).not.toHaveBeenCalled();
});

it("owned workbench forwards canonical edits and uses its existing session channel", () => {
  const active = channel(), updateRequest = vi.fn();
  owned.state = { authenticated: true, accountId: "owner", draft: { id: "owned", data: { active_environment_id: null } }, request: request(), dark: false, protocolSession: active, updateRequest };
  render(<Theme><MqttWorkbench /></Theme>);
  fireEvent.change(screen.getByLabelText("MQTT 消息 Payload"), { target: { value: "owned edit" } });
  expect(updateRequest).toHaveBeenCalledWith({ protocol: expect.objectContaining({ message: expect.objectContaining({ payload_source: "owned edit" }) }) });
  fireEvent.click(screen.getByRole("button", { name: "发布" }));
  expect(active.send).toHaveBeenCalledWith({ kind: "mqtt_publish", message: expect.objectContaining({ payload_source: "saved payload" }) });
});

it("retained saved-message selection cannot put an old name into a changed scope", () => {
  const old = request(); if (old.protocol?.kind !== "mqtt") throw new Error();
  old.protocol.saved_messages = [{ id: "shared-id", name: "previous account label", message: structuredClone(old.protocol.message) }];
  const onChange = vi.fn(), props = { dark: false, isCurrent: () => true, onChange };
  const { rerender } = render(<Theme><MqttClient {...props} request={old} scope="old-account" /></Theme>);
  const retained = callbacks.saved!;
  rerender(<Theme><MqttClient {...props} request={request()} scope="new-account" /></Theme>);
  act(() => retained("shared-id"));
  expect(onChange).not.toHaveBeenCalled();
  expect((screen.getByLabelText("消息名称") as HTMLInputElement).value).toBe("");
});
