// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { Theme } from "@radix-ui/themes";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { api, token } from "../../shared/api";
import { setLanguage } from "../../shared/i18n";
import { initialData } from "../../shared/model";
import SharedLiveSessionPanel from "./SharedLiveSessionPanel";
vi.mock("../../shared/api", () => ({ native: false,  api: vi.fn(), token: vi.fn() }));
vi.mock("../protocols/SessionEventPane", () => ({ default: () => <div>Events</div> }));
const base = "/api/owners/owner/workspaces/shared";
const session = { id: "a", protocol: "websocket", state: "open", url: "wss://fixture.test", received_bytes: 0, sent_bytes: 0, handshake: { status: 101, headers: [] } };
let valid: boolean;
function view(saving = false) {
  const request = { ...initialData().collections[0].requests[0], protocol: { kind: "websocket" as const } };
  return <Theme><SharedLiveSessionPanel {...{ base, request, revision: 3, environmentId: null, dirty: false, saving, conflict: false, dark: false, isCurrent: () => valid, onSaveResultDataset: async () => {} }} /></Theme>;
}
beforeEach(async () => { vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} }); await setLanguage("zh-CN"); valid = true; vi.mocked(token).mockReturnValue("member"); vi.mocked(api).mockReset(); vi.mocked(api).mockImplementation((path) => Promise.resolve(path.includes("/events?") ? { events: [], next_cursor: 0, earliest_cursor: 0, dropped_count: 0 } : session)); });
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });
it("does not create a session after its authoritative cache admission changes before click", () => {
  render(view()); valid = false;
  fireEvent.click(screen.getByRole("button", { name: "连接" }));
  expect(api).not.toHaveBeenCalled();
});
it("closes a late created session and withholds its handshake after admission changes", async () => {
  let finish!: (value: unknown) => void;
  const fetch = vi.fn().mockResolvedValue({}); vi.stubGlobal("fetch", fetch);
  vi.mocked(api).mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
  render(view()); fireEvent.click(screen.getByRole("button", { name: "连接" }));
  valid = false; await act(async () => finish(session));
  expect(screen.queryByRole("button", { name: "断开" })).toBeNull();
  expect(fetch).toHaveBeenCalledWith(`${base}/sessions/a/close`, expect.objectContaining({ method: "POST", headers: { Authorization: "Bearer member" } }));
});
it("does not read private events if admission changes during summary loading", async () => {
  let finish!: (value: unknown) => void;
  vi.mocked(api).mockImplementation((path) => path === `${base}/sessions` ? Promise.resolve(session) : new Promise((resolve) => { finish = resolve; }));
  render(view()); fireEvent.click(screen.getByRole("button", { name: "连接" }));
  await waitFor(() => expect(finish).toBeTypeOf("function"));
  valid = false; await act(async () => finish(session));
  expect(vi.mocked(api).mock.calls.some(([path]) => path.includes("/events?"))).toBe(false);
});
it("does not send through a previously mounted session after cache admission changes", async () => {
  render(view()); fireEvent.click(screen.getByRole("button", { name: "连接" }));
  fireEvent.change(await screen.findByLabelText("消息正文"), { target: { value: "stale" } });
  valid = false; fireEvent.click(screen.getByRole("button", { name: "发送消息" }));
  expect(vi.mocked(api).mock.calls.some(([path]) => path.endsWith("/send"))).toBe(false);
});

it("closes the original live session when its cache context becomes invalid during polling", async () => {
  const fetch = vi.fn().mockResolvedValue({}); vi.stubGlobal("fetch", fetch);
  render(view()); fireEvent.click(screen.getByRole("button", { name: "连接" }));
  await screen.findByRole("button", { name: "断开" });
  valid = false;
  await waitFor(() => expect(fetch).toHaveBeenCalledWith(`${base}/sessions/a/close`, expect.objectContaining({ method: "POST", headers: { Authorization: "Bearer member" } })), { timeout: 2500 });
  expect(screen.queryByRole("button", { name: "断开" })).toBeNull();
});
it("keeps a saved session polling while a workspace Save is pending", async () => {
  const fetch = vi.fn().mockResolvedValue({}); vi.stubGlobal("fetch", fetch);
  const { rerender } = render(view()); fireEvent.click(screen.getByRole("button", { name: "连接" }));
  await screen.findByRole("button", { name: "断开" });
  await waitFor(() => expect(vi.mocked(api).mock.calls.some(([path]) => path.includes("/events?"))).toBe(true));
  const previous = vi.mocked(api).mock.calls.filter(([path]) => path.includes("/events?")).length;
  rerender(view(true));
  await waitFor(() => expect(vi.mocked(api).mock.calls.filter(([path]) => path.includes("/events?")).length).toBeGreaterThan(previous), { timeout: 2500 });
  expect(screen.getByRole("button", { name: "断开" })).toBeTruthy();
  expect(fetch).not.toHaveBeenCalled();
});

it("does not reopen a closed session when an older in-flight poll finishes later", async () => {
  let finishSummary!: (value: unknown) => void;
  vi.mocked(api).mockImplementation((path) => {
    if (path === `${base}/sessions`) return Promise.resolve(session);
    if (path === `${base}/sessions/a`) return new Promise((resolve) => { finishSummary = resolve; });
    if (path.endsWith("/close")) return Promise.resolve({ ...session, state: "closed" });
    return Promise.resolve({ events: [], next_cursor: 0, earliest_cursor: 0, dropped_count: 0 });
  });
  render(view()); fireEvent.click(screen.getByRole("button", { name: "连接" }));
  await waitFor(() => expect(finishSummary).toBeTypeOf("function"));
  fireEvent.click(screen.getByRole("button", { name: "断开" }));
  await screen.findByRole("button", { name: "重新连接" });
  await act(async () => finishSummary(session));
  expect(screen.queryByRole("button", { name: "断开" })).toBeNull();
  expect(screen.getByRole("button", { name: "重新连接" })).toBeTruthy();
});
