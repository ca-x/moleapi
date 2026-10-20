// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Theme } from "@radix-ui/themes";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { api, previewSharedMock, token } from "../../shared/api";
import { setLanguage } from "../../shared/i18n";
import { initialData } from "../../shared/model";
import { dataConfig } from "../data/model";
import { formatRequestLink } from "../documents/internalLinks";
import { saveProjectFile } from "../generation/saveProjectFile";
import type { SharedWorkspaceSummary, Workspace } from "../../shared/types";
import SharedWorkspaceView from "./SharedWorkspaceView";

vi.mock("../../shared/api", () => ({ api: vi.fn(), previewSharedMock: vi.fn(), token: vi.fn() }));
vi.mock("../generation/saveProjectFile", () => ({ saveProjectFile: vi.fn().mockResolvedValue(undefined) }));
vi.mock("../../shared/ui", () => ({
  Field: ({ label, children }: { label: string; children: React.ReactNode }) => <label>{label}{children}</label>,
  Choice: ({ label, value, options, onChange, disabled }: { disabled?: boolean; label: string; value: string; options: { value: string; label: string }[]; onChange: (value: string) => void }) => <label>{label}<select disabled={disabled} value={value} onChange={(event) => onChange(event.target.value)}>{options.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</select></label>,
  Editor: ({ label, value, onChange }: { label: string; value: string; onChange: (value: string) => void }) => <label>{label}<textarea value={value} onChange={(event) => onChange(event.target.value)} /></label>,
  PairEditor: ({ rows, onChange, keyLabel, valueLabel }: { rows: { id: string; key: string; value: string; enabled: boolean }[]; onChange: (rows: { id: string; key: string; value: string; enabled: boolean }[]) => void; keyLabel: string; valueLabel: string }) => <div>{rows.map((row) => <div key={row.id}><label>{keyLabel}<input value={row.key} onChange={(event) => onChange(rows.map((item) => item.id === row.id ? { ...item, key: event.target.value } : item))} /></label><label>{valueLabel}<input value={row.value} onChange={(event) => onChange(rows.map((item) => item.id === row.id ? { ...item, value: event.target.value } : item))} /></label></div>)}<button onClick={() => onChange([...rows, { id: "header-a", key: "X-Test", value: "yes", enabled: true }])}>添加一行</button></div>,
  ToolButton: ({ label, onClick, children }: { label: string; onClick: () => void; children: React.ReactNode }) => <button aria-label={label} onClick={onClick}>{children}</button>,
}));
const base = "/api/owners/owner-id/workspaces/w";
const workspace: Workspace = { id: "w", name: "Team", revision: 3, updated_at: "now", data: initialData() };
const summary = (role: SharedWorkspaceSummary["role"]): SharedWorkspaceSummary => ({ owner_id: "owner-id", owner_username: "owner", workspace_id: "w", workspace_name: "Team", revision: 3, role });
let client: QueryClient;
function view(role: SharedWorkspaceSummary["role"], accountId = "member") { return <QueryClientProvider client={client}><Theme><SharedWorkspaceView accountId={accountId} selected={summary(role)} dark={false} onClose={vi.fn()} onDirtyChange={vi.fn()} /></Theme></QueryClientProvider>; }
beforeEach(async () => { vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} }); await setLanguage("zh-CN"); client = new QueryClient({ defaultOptions: { queries: { retry: false } } }); vi.mocked(token).mockReturnValue("session-member"); vi.mocked(api).mockReset(); vi.mocked(api).mockResolvedValue(workspace); vi.mocked(previewSharedMock).mockReset(); vi.mocked(saveProjectFile).mockReset(); vi.mocked(saveProjectFile).mockResolvedValue(undefined); });
afterEach(() => { cleanup(); client.clear(); vi.unstubAllGlobals(); });

it("keeps viewer resources read-only and hides private execution controls", async () => {
  render(view("viewer"));
  await screen.findByLabelText("请求");
  expect(screen.getByText("此工作区为只读。不能保存、运行请求或查看私有执行记录。")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "保存共享工作区" })).toBeNull();
  expect(screen.queryByRole("button", { name: "执行历史" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Mock 预览" })).toBeNull();
  expect(screen.queryByRole("tab", { name: /参数/ })).toBeNull();
  expect(api).toHaveBeenCalledWith(base);
});

it("downloads a saved redacted report through the member route", async () => {
  vi.mocked(api).mockImplementation((path) => {
    if (path === `${base}/reports`) return Promise.resolve({ items: [{ id: "report-a", collection_name: "Team tests", summary: { passed: 1, failed: 0 } }], next_cursor: null });
    if (path.includes("/export?")) return Promise.resolve({ filename: "report.json", content: '{"safe":true}', mime: "application/json" });
    return Promise.resolve(workspace);
  });
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "测试报告" }));
  await screen.findByText("Team tests");
  fireEvent.change(screen.getByLabelText("报告导出格式"), { target: { value: "json" } });
  fireEvent.click(screen.getByRole("button", { name: "下载脱敏报告" }));
  await waitFor(() => expect(saveProjectFile).toHaveBeenCalledTimes(1));
  expect(api).toHaveBeenCalledWith(`${base}/reports/report-a/export?format=json&language=zh-CN`);
  expect(vi.mocked(saveProjectFile).mock.calls[0][0]).toBe("report.json");
});

it("stops the active shared collection using its original client job ID", async () => {
  let finish!: (value: unknown) => void;
  vi.mocked(api).mockImplementation((path) => {
    if (path === `${base}/run`) return new Promise((resolve) => { finish = resolve; });
    if (path === `${base}/run/cancel`) return Promise.resolve({ cancelled: true });
    return Promise.resolve(workspace);
  });
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "运行集合" }));
  await waitFor(() => expect(finish).toBeTypeOf("function"));
  const started = vi.mocked(api).mock.calls.find(([path]) => path === `${base}/run`)?.[2] as { job_id: string };
  expect(started.job_id).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "停止集合运行" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(`${base}/run/cancel`, "POST", { job_id: started.job_id }));
  await act(async () => finish({ passed: 0, failed: 0, cancelled: true }));
  expect(await screen.findByText(/已取消/)).toBeTruthy();
  expect(screen.queryByRole("button", { name: "停止集合运行" })).toBeNull();
});

it.each(["role", "account"] as const)("discards a delayed shared report export after the %s changes", async (boundary) => {
  let finish!: (value: unknown) => void;
  vi.mocked(api).mockImplementation((path) => {
    if (path === `${base}/reports`) return Promise.resolve({ items: [{ id: "report-a", collection_name: "Team tests", summary: { passed: 1, failed: 0 } }], next_cursor: null });
    if (path.includes("/export?")) return new Promise((resolve) => { finish = resolve; });
    return Promise.resolve(workspace);
  });
  const { rerender } = render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "测试报告" }));
  fireEvent.click(await screen.findByRole("button", { name: "下载脱敏报告" }));
  await waitFor(() => expect(finish).toBeTypeOf("function"));
  rerender(boundary === "role" ? view("viewer") : view("editor", "other-member"));
  await act(async () => finish({ filename: "private.json", content: "private", mime: "application/json" }));
  expect(saveProjectFile).not.toHaveBeenCalled();
  expect(screen.queryByRole("button", { name: "下载脱敏报告" })).toBeNull();
});

it("connects a saved shared WebSocket, sends a message and keeps viewer controls absent", async () => {
  const first = workspace.data.collections[0];
  const request = { ...first.requests[0], protocol: { kind: "websocket" as const }, url: "ws://127.0.0.1/socket" };
  const saved: Workspace = { ...workspace, data: { ...workspace.data, collections: [{ ...first, requests: [request] }] } };
  const session = { id: "live-a", workspace_id: "w", request_id: request.id, protocol: "websocket", url: "ws://127.0.0.1/socket", state: "open", reason: null, created_at: "now", updated_at: "now", received_bytes: 5, sent_bytes: 0, event_count: 1, handshake: null };
  let revoked = false;
  vi.mocked(api).mockImplementation((path, method) => {
    if (path === `${base}/sessions` && method === "POST") return Promise.resolve(session);
    if (path === `${base}/sessions/live-a` && method === "DELETE") return Promise.resolve({ ok: true });
    if (path === `${base}/sessions/live-a`) return revoked ? Promise.reject(Object.assign(new Error("Revoked"), { status: 404 })) : Promise.resolve(session);
    if (path.startsWith(`${base}/sessions/live-a/events`)) return Promise.resolve({ events: [{ cursor: 1, received_at: "now", direction: "incoming", message: { kind: "text", text: "hello" } }], next_cursor: 1, earliest_cursor: 1, dropped_count: 0 });
    if (path.endsWith("/send")) return Promise.resolve({ ok: true });
    if (path.endsWith("/close")) return Promise.resolve({ ...session, state: "closed" });
    return Promise.resolve(saved);
  });
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "连接" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(`${base}/sessions`, "POST", { request_id: request.id, environment_id: saved.data.active_environment_id, expected_revision: 3 }));
  expect((await screen.findAllByText("hello")).length).toBeGreaterThan(0);
  fireEvent.change(screen.getByLabelText("消息正文"), { target: { value: "member echo" } });
  fireEvent.click(screen.getByRole("button", { name: "发送消息" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(`${base}/sessions/live-a/send`, "POST", { kind: "text", text: "member echo" }));
  fireEvent.click(screen.getByRole("button", { name: "断开" }));
  fireEvent.click(await screen.findByRole("button", { name: "重新连接" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(`${base}/sessions/live-a`, "DELETE"));
  fireEvent.click(await screen.findByRole("button", { name: "连接" }));
  await screen.findByRole("button", { name: "发送消息" });
  revoked = true;
  await waitFor(() => expect(screen.queryAllByText("hello")).toHaveLength(0), { timeout: 3000 });
  cleanup();
  render(view("viewer"));
  await screen.findByText("ws://127.0.0.1/socket");
  expect(screen.queryByRole("button", { name: "连接" })).toBeNull();
});

it("shows only projected Markdown to a viewer and keeps member publication controls absent", async () => {
  const safe: Workspace = { ...workspace, data: { ...workspace.data, documents: [{ id: "private-doc", name: "Private", source: "", secret: true }, { id: "public-doc", name: "Public", source: "# Safe heading", secret: false }] } };
  vi.mocked(api).mockResolvedValue(safe);
  render(view("viewer"));
  fireEvent.click(await screen.findByRole("button", { name: "文档" }));
  expect(screen.getByText("此文档正文未向查看者开放。")).toBeTruthy();
  fireEvent.change(screen.getByLabelText("文档"), { target: { value: "public-doc" } });
  expect(screen.getByRole("heading", { name: "Safe heading" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "新建文档" })).toBeNull();
  expect(screen.queryByRole("button", { name: "删除文档" })).toBeNull();
  expect(screen.queryByRole("button", { name: "保存共享工作区" })).toBeNull();
  expect(screen.queryByRole("button", { name: "发布" })).toBeNull();
});

it("opens only a request in the current shared workspace from a Markdown link", async () => {
  const requestId = workspace.data.collections[0].requests[0].id;
  const linked: Workspace = { ...workspace, data: { ...workspace.data, documents: [{ id: "guide", name: "Guide", source: `[Open request](${formatRequestLink(requestId)}) [Unknown](moleapi://request/foreign-id)`, secret: false }] } };
  vi.mocked(api).mockResolvedValue(linked);
  render(view("viewer"));
  fireEvent.click(await screen.findByRole("button", { name: "文档" }));
  expect(screen.queryByRole("button", { name: "Unknown" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Open request" }));
  expect(screen.getByText(workspace.data.collections[0].requests[0].url)).toBeTruthy();
});

it("creates, previews and saves a private shared Markdown document through CAS", async () => {
  vi.mocked(api).mockImplementation((_path, method, body) => Promise.resolve(method === "PUT" ? { ...workspace, revision: 4, data: (body as { data: Workspace["data"] }).data } : workspace));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "文档" }));
  fireEvent.click(screen.getByRole("button", { name: "新建文档" }));
  fireEvent.change(screen.getByLabelText("文档标题"), { target: { value: "Team guide" } });
  fireEvent.change(screen.getByLabelText("Markdown 正文"), { target: { value: "# Team guide\n\nSafe **details**" } });
  fireEvent.mouseDown(screen.getByRole("tab", { name: /预览/ }), { button: 0 });
  expect(screen.getAllByRole("heading", { name: "Team guide" }).length).toBeGreaterThan(1);
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", expect.objectContaining({ expected_revision: 3 })));
  const saved = vi.mocked(api).mock.calls.find(([path, method]) => path === base && method === "PUT")?.[2] as { data: Workspace["data"] };
  expect(saved.data.documents?.[0]).toMatchObject({ name: "Team guide", source: "# Team guide\n\nSafe **details**", secret: true });
});

it("blocks every shared Save entry while a Markdown document has an invalid title", async () => {
  const withDocument: Workspace = { ...workspace, data: { ...workspace.data, documents: [{ id: "doc", name: "Valid", source: "", secret: true }] } };
  vi.mocked(api).mockImplementation((path) => Promise.resolve(path.endsWith("/mock-calls") ? [] : withDocument));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "文档" }));
  fireEvent.change(screen.getByLabelText("文档标题"), { target: { value: " Invalid" } });
  const documentError = "文档「 Invalid」的标题须为 1–128 个字符，不能含首尾空格或控制字符。";
  expect(screen.getByText(documentError)).toBeTruthy();
  expect(screen.getByRole("button", { name: "保存共享工作区" })).toHaveProperty("disabled", true);
  fireEvent.click(screen.getByRole("button", { name: "资源" }));
  expect(screen.getByText(documentError)).toBeTruthy();
  expect(screen.getByRole("button", { name: "保存共享工作区" })).toHaveProperty("disabled", true);
  fireEvent.click(screen.getByRole("button", { name: "Mock 预览" }));
  expect(screen.getByText(documentError)).toBeTruthy();
  expect(screen.getByRole("button", { name: "保存共享工作区" })).toHaveProperty("disabled", true);
  expect(vi.mocked(api).mock.calls.some(([, method]) => method === "PUT")).toBe(false);
});

it("asks before removing a shared document and preserves the draft on cancel", async () => {
  const withDocument: Workspace = { ...workspace, data: { ...workspace.data, documents: [{ id: "doc", name: "Team guide", source: "# Guide", secret: true }] } };
  vi.mocked(api).mockResolvedValue(withDocument);
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "文档" }));
  fireEvent.click(screen.getByRole("button", { name: "删除文档" }));
  expect(screen.getByText("删除「Team guide」？保存工作区后生效。")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(screen.getByRole("heading", { name: "Team guide" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "删除文档" }));
  fireEvent.click(screen.getByRole("button", { name: "确认删除" }));
  expect(screen.getByText("还没有 Markdown 文档。")).toBeTruthy();
  expect(screen.getByRole("button", { name: "保存共享工作区" })).toHaveProperty("disabled", false);
});

it("closes a document deletion confirmation when the owner changes that document", async () => {
  const withDocument: Workspace = { ...workspace, data: { ...workspace.data, documents: [{ id: "doc", name: "Old guide", source: "# Old", secret: true }] } };
  vi.mocked(api).mockResolvedValue(withDocument);
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "文档" }));
  fireEvent.click(screen.getByRole("button", { name: "删除文档" }));
  expect(screen.getByText("删除「Old guide」？保存工作区后生效。")).toBeTruthy();
  const remote: Workspace = { ...withDocument, revision: 4, data: { ...withDocument.data, documents: [{ id: "doc", name: "New guide", source: "# New", secret: true }] } };
  await act(async () => client.setQueryData(["shared-workspace", "member", "owner-id", "w", "editor", 0], remote));
  await waitFor(() => expect(screen.queryByText("删除「Old guide」？保存工作区后生效。")).toBeNull());
  expect(screen.getByRole("heading", { name: "New guide" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "确认删除" })).toBeNull();
  expect(screen.getByRole("button", { name: "保存共享工作区" })).toHaveProperty("disabled", true);
});

it("shows projected shared variables to a viewer without editing or private values", async () => {
  const projected: Workspace = { ...workspace, data: { ...workspace.data, global_variables: [{ id: "secret", key: "token", value: "", enabled: true, secret: true }, { id: "inferred", key: "api_key", value: "", enabled: true, secret: false }, { id: "public", key: "region", value: "west", enabled: true }], environments: [{ id: "dev", name: "Dev", variables: [{ id: "env", key: "host", value: "local", enabled: true }] }], active_environment_id: "dev" } };
  vi.mocked(api).mockResolvedValue(projected);
  render(view("viewer"));
  fireEvent.click(await screen.findByRole("button", { name: "环境变量" }));
  expect(screen.getByText("host")).toBeTruthy();
  expect(screen.getByText("这里只显示工作区共享值；私密值已隐藏。")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "新建环境" })).toBeNull();
  expect(screen.queryByRole("button", { name: "保存共享工作区" })).toBeNull();
  fireEvent.mouseDown(screen.getByRole("tab", { name: /项目全局/ }), { button: 0 });
  expect(screen.getByText("私密值已隐藏")).toBeTruthy();
  expect(screen.getByText("空值或已隐藏")).toBeTruthy();
  expect(screen.getByText("west")).toBeTruthy();
  expect(screen.queryByLabelText("共享值")).toBeNull();
});

it("saves shared environment, project and collection variables through owner CAS", async () => {
  const first = workspace.data.collections[0];
  const seeded: Workspace = { ...workspace, data: { ...workspace.data, global_variables: [{ id: "global", key: "region", value: "old", enabled: true, local_value: "owner-local" }], collections: [{ ...first, variables: [{ id: "collection", key: "scope", value: "old", enabled: true, local_value: "owner-local" }] }], environments: workspace.data.environments.map((item) => ({ ...item, variables: item.variables.map((row) => ({ ...row, local_value: "owner-local" })) })) } };
  vi.mocked(api).mockImplementation((_path, method, body) => Promise.resolve(method === "PUT" ? { ...seeded, revision: 4, data: (body as { data: Workspace["data"] }).data } : seeded));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "环境变量" }));
  fireEvent.change(screen.getByLabelText("环境名称"), { target: { value: "Team dev" } });
  fireEvent.change(screen.getAllByLabelText("共享值")[0], { target: { value: "https://shared.example" } });
  fireEvent.mouseDown(screen.getByRole("tab", { name: /项目全局/ }), { button: 0 });
  fireEvent.change(screen.getByLabelText("共享值"), { target: { value: "west" } });
  fireEvent.mouseDown(screen.getByRole("tab", { name: /集合变量/ }), { button: 0 });
  fireEvent.change(screen.getByLabelText("共享值"), { target: { value: "team" } });
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", expect.objectContaining({ expected_revision: 3 })));
  const saved = vi.mocked(api).mock.calls.find(([path, method]) => path === base && method === "PUT")?.[2] as { data: Workspace["data"] };
  expect(saved.data.environments[0].name).toBe("Team dev");
  expect(saved.data.environments[0].variables[0].value).toBe("https://shared.example");
  expect(saved.data.global_variables?.[0].value).toBe("west");
  expect(saved.data.collections[0].variables?.[0].value).toBe("team");
  expect(saved.data.global_variables?.[0].local_value).toBeUndefined();
  expect(saved.data.collections[0].variables?.[0].local_value).toBeUndefined();
  expect(saved.data.environments[0].variables[0].local_value).toBeUndefined();
});

it("creates and selects a shared environment, then confirms its draft removal", async () => {
  vi.mocked(api).mockResolvedValue(workspace);
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "环境变量" }));
  fireEvent.click(screen.getByRole("button", { name: "新建环境" }));
  fireEvent.change(screen.getByLabelText("环境名称"), { target: { value: "Staging" } });
  expect(screen.getByRole("button", { name: "保存共享工作区" })).toHaveProperty("disabled", false);
  fireEvent.click(screen.getByRole("button", { name: "删除当前环境" }));
  expect(screen.getByText("删除「Staging」？保存工作区后生效。")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect((screen.getByLabelText("环境名称") as HTMLInputElement).value).toBe("Staging");
  fireEvent.click(screen.getByRole("button", { name: "删除当前环境" }));
  fireEvent.click(screen.getByRole("button", { name: "确认删除" }));
  expect((screen.getByLabelText("环境") as HTMLSelectElement).value).toBe("local");
  expect(screen.getByRole("button", { name: "保存共享工作区" })).toHaveProperty("disabled", true);
});

it("closes environment deletion confirmation after a remote revision", async () => {
  vi.mocked(api).mockResolvedValue(workspace);
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "环境变量" }));
  fireEvent.click(screen.getByRole("button", { name: "删除当前环境" }));
  expect(screen.getByText("删除「Local development」？保存工作区后生效。")).toBeTruthy();
  const remote: Workspace = { ...workspace, revision: 4, data: { ...workspace.data, environments: [{ ...workspace.data.environments[0], name: "Owner renamed" }] } };
  await act(async () => client.setQueryData(["shared-workspace", "member", "owner-id", "w", "editor", 0], remote));
  await waitFor(() => expect(screen.queryByRole("button", { name: "确认删除" })).toBeNull());
  expect(screen.getByText("Owner renamed")).toBeTruthy();
  expect(screen.getByRole("button", { name: "保存共享工作区" })).toHaveProperty("disabled", true);
});

it("closes a stale environment deletion confirmation when a dirty draft conflicts", async () => {
  vi.mocked(api).mockImplementation((_path, method, body) => Promise.resolve(method === "PUT" ? { ...workspace, revision: 5, data: (body as { data: Workspace["data"] }).data } : workspace));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "环境变量" }));
  fireEvent.change(screen.getByLabelText("环境名称"), { target: { value: "Member edit" } });
  fireEvent.click(screen.getByRole("button", { name: "删除当前环境" }));
  expect(screen.getByRole("button", { name: "确认删除" })).toBeTruthy();
  const remote: Workspace = { ...workspace, revision: 4, data: { ...workspace.data, environments: [{ ...workspace.data.environments[0], name: "Owner edit" }] } };
  await act(async () => client.setQueryData(["shared-workspace", "member", "owner-id", "w", "editor", 0], remote));
  expect(await screen.findByText("共享工作区已被其他成员更新。选择要保留的版本。")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "确认删除" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "保留我的修改" }));
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", expect.objectContaining({ expected_revision: 4 })));
  const saved = vi.mocked(api).mock.calls.find(([path, method]) => path === base && method === "PUT")?.[2] as { data: Workspace["data"] };
  expect(saved.data.environments[0].name).toBe("Member edit");
  expect(saved.data.environments).toHaveLength(1);
});

it("keeps a shared Markdown draft through a remote revision conflict", async () => {
  const withDocument: Workspace = { ...workspace, data: { ...workspace.data, documents: [{ id: "doc", name: "Guide", source: "# Original", secret: true }] } };
  vi.mocked(api).mockImplementation((_path, method, body) => Promise.resolve(method === "PUT" ? { ...withDocument, revision: 5, data: (body as { data: Workspace["data"] }).data } : withDocument));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "文档" }));
  fireEvent.mouseDown(screen.getByRole("tab", { name: /编辑/ }), { button: 0 });
  fireEvent.change(screen.getByLabelText("Markdown 正文"), { target: { value: "# Member draft" } });
  fireEvent.click(screen.getByRole("button", { name: "删除文档" }));
  expect(screen.getByRole("button", { name: "确认删除" })).toBeTruthy();
  const remote = { ...withDocument, revision: 4, data: { ...withDocument.data, documents: [{ id: "doc", name: "Guide", source: "# Owner change", secret: true }] } };
  await act(async () => client.setQueryData(["shared-workspace", "member", "owner-id", "w", "editor", 0], remote));
  expect(await screen.findByText("共享工作区已被其他成员更新。选择要保留的版本。")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "确认删除" })).toBeNull();
  expect((screen.getByLabelText("Markdown 正文") as HTMLTextAreaElement).value).toBe("# Member draft");
  expect(screen.getByRole("button", { name: "保存共享工作区" })).toHaveProperty("disabled", true);
  fireEvent.click(screen.getByRole("button", { name: "保留我的修改" }));
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", expect.objectContaining({ expected_revision: 4 })));
  const saved = vi.mocked(api).mock.calls.find(([path, method]) => path === base && method === "PUT")?.[2] as { data: Workspace["data"] };
  expect(saved.data.documents?.[0].source).toBe("# Member draft");
});

it("previews a saved shared Mock example and loads its calls for an editor", async () => {
  const first = workspace.data.collections[0].requests[0];
  const example = { id: "example-a", name: "Success", status: 201, headers: [], body: "saved" };
  const withExample: Workspace = { ...workspace, data: { ...workspace.data, collections: [{ ...workspace.data.collections[0], requests: [{ ...first, examples: [example] }] }] } };
  vi.mocked(api).mockImplementation((path) => Promise.resolve(path.endsWith("/mock-calls") ? [{ id: "call-a", request_id: first.id, example_id: example.id, response_status: 201, outcome: "served", at: "2026-10-19T00:00:00Z" }] : withExample));
  vi.mocked(previewSharedMock).mockResolvedValue({ status: 201, body: "shared-mock" });
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "Mock 预览" }));
  fireEvent.click(await screen.findByRole("button", { name: "预览已保存 Mock 示例" }));
  await waitFor(() => expect(previewSharedMock).toHaveBeenCalledWith(`${base}/mock/${encodeURIComponent(first.id)}/${example.id}`));
  expect((await screen.findByLabelText("Mock 响应正文") as HTMLTextAreaElement).value).toBe("shared-mock");
  expect(screen.getByText(`请求: ${first.id} · 响应示例: ${example.id}`)).toBeTruthy();
  expect(screen.getByText("201 · 已响应")).toBeTruthy();
});

it("saves example edits through owner CAS before previewing the saved version", async () => {
  const first = workspace.data.collections[0].requests[0];
  const example = { id: "example-a", name: "Old", status: 200, headers: [], body: "old", mock_script: "console.log('keep')" };
  const withExample: Workspace = { ...workspace, data: { ...workspace.data, collections: [{ ...workspace.data.collections[0], requests: [{ ...first, examples: [example] }] }] } };
  vi.mocked(api).mockImplementation((path, method, body) => {
    if (path.endsWith("/mock-calls")) return Promise.resolve([]);
    if (method === "PUT") return Promise.resolve({ ...withExample, revision: 4, data: (body as { data: Workspace["data"] }).data });
    return Promise.resolve(withExample);
  });
  vi.mocked(previewSharedMock).mockResolvedValue({ status: 502, body: "simulated failure" });
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "Mock 预览" }));
  fireEvent.change(screen.getByLabelText("示例名称"), { target: { value: "Edited" } });
  fireEvent.change(screen.getByLabelText("状态码"), { target: { value: "401" } });
  fireEvent.change(screen.getByLabelText("Mock 延迟（毫秒）"), { target: { value: "500" } });
  fireEvent.change(screen.getByLabelText("故障比例（%）"), { target: { value: "100" } });
  fireEvent.change(screen.getByLabelText("故障状态码（5xx）"), { target: { value: "502" } });
  fireEvent.change(screen.getByLabelText("每分钟请求上限"), { target: { value: "2" } });
  fireEvent.change(screen.getByLabelText("响应正文"), { target: { value: "saved edited body" } });
  fireEvent.click(screen.getByRole("button", { name: "添加一行" }));
  expect(screen.getByRole("button", { name: "预览已保存 Mock 示例" })).toHaveProperty("disabled", true);
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", expect.objectContaining({ expected_revision: 3 })));
  const saved = vi.mocked(api).mock.calls.find(([path, method]) => path === base && method === "PUT")?.[2] as { data: Workspace["data"] };
  expect(saved.data.collections[0].requests[0].examples[0]).toMatchObject({ id: "example-a", name: "Edited", status: 401, delay_ms: 500, fault_rate_percent: 100, fault_status: 502, rate_limit_per_minute: 2, body: "saved edited body", mock_script: "console.log('keep')", headers: [{ key: "X-Test", value: "yes" }] });
  await waitFor(() => expect(screen.getByRole("button", { name: "预览已保存 Mock 示例" })).toHaveProperty("disabled", false));
  fireEvent.click(screen.getByRole("button", { name: "预览已保存 Mock 示例" }));
  await waitFor(() => expect(previewSharedMock).toHaveBeenCalledWith(`${base}/mock/${encodeURIComponent(first.id)}/example-a`));
});

it("acknowledges canonical Mock defaults after disabling faults and limits", async () => {
  const first = workspace.data.collections[0].requests[0];
  const withExample: Workspace = { ...workspace, data: { ...workspace.data, collections: [{ ...workspace.data.collections[0], requests: [{ ...first, examples: [{ id: "example-a", name: "Traffic", status: 200, headers: [], body: "static", fault_rate_percent: 100, rate_limit_per_minute: 1 }] }] }] } };
  vi.mocked(api).mockImplementation((path, method, body) => {
    if (path.endsWith("/mock-calls")) return Promise.resolve([]);
    if (method === "PUT") {
      const data = structuredClone((body as { data: Workspace["data"] }).data);
      delete data.collections[0].requests[0].examples[0].fault_rate_percent;
      delete data.collections[0].requests[0].examples[0].rate_limit_per_minute;
      return Promise.resolve({ ...withExample, revision: 4, data });
    }
    return Promise.resolve(withExample);
  });
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "Mock 预览" }));
  fireEvent.change(screen.getByLabelText("故障比例（%）"), { target: { value: "0" } });
  fireEvent.change(screen.getByLabelText("每分钟请求上限"), { target: { value: "0" } });
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "预览已保存 Mock 示例" })).toHaveProperty("disabled", false));
  expect(screen.getByRole("button", { name: "保存共享工作区" })).toHaveProperty("disabled", true);
  expect(screen.getByLabelText("故障比例（%）")).toHaveProperty("value", "0");
});

it("edits shared inline Mock Schema and inference through canonical CAS", async () => {
  const first = workspace.data.collections[0].requests[0];
  const withExample: Workspace = { ...workspace, data: { ...workspace.data, collections: [{ ...workspace.data.collections[0], requests: [{ ...first, examples: [{ id: "schema-example", name: "Schema", status: 200, headers: [], body: "static body", fault_rate_percent: 17 }] }] }] } };
  vi.mocked(api).mockImplementation((path, method, body) => path.endsWith("/mock-calls") ? Promise.resolve([]) : Promise.resolve(method === "PUT" ? { ...withExample, revision: 4, data: (body as { data: Workspace["data"] }).data } : withExample));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "Mock 预览" }));
  fireEvent.change(screen.getByLabelText("Mock 正文来源"), { target: { value: "inline" } });
  fireEvent.click(screen.getByRole("button", { name: "编辑 Mock JSON Schema" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Mock JSON Schema" }), { target: { value: "invalid" } });
  expect(screen.getByRole("button", { name: "完成" })).toHaveProperty("disabled", true);
  const schema = '{"type":"object","properties":{"id":{"const":7}},"required":["id"]}';
  fireEvent.change(screen.getByRole("textbox", { name: "Mock JSON Schema" }), { target: { value: schema } });
  fireEvent.click(screen.getByRole("button", { name: "完成" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "根据字段名生成 Mock 值" }));
  expect(screen.getByRole("button", { name: "预览已保存 Mock 示例" })).toHaveProperty("disabled", true);
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", expect.objectContaining({ expected_revision: 3 })));
  const sent = vi.mocked(api).mock.calls.find(([path, method]) => path === base && method === "PUT")?.[2] as { data: Workspace["data"] };
  expect(sent.data.collections[0].requests[0].examples[0]).toMatchObject({ mock_schema: schema, mock_name_inference: true, body: "static body", fault_rate_percent: 17 });
  expect(vi.mocked(api).mock.calls.some(([path]) => path.includes("/mock-schema/validate"))).toBe(false);
});

it.each([false, true])("closes an uncommitted shared Schema dialog on remote revision (dirty=%s)", async (dirty) => {
  const first = workspace.data.collections[0].requests[0];
  const withExample: Workspace = { ...workspace, data: { ...workspace.data, collections: [{ ...workspace.data.collections[0], requests: [{ ...first, examples: [{ id: "schema-example", name: "Schema", status: 200, headers: [], body: "static", mock_schema: '{"type":"string","const":"old"}' }] }] }] } };
  vi.mocked(api).mockImplementation((path) => Promise.resolve(path.endsWith("/mock-calls") ? [] : withExample));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "Mock 预览" }));
  if (dirty) fireEvent.change(screen.getByLabelText("示例名称"), { target: { value: "Unsaved title" } });
  fireEvent.click(screen.getByRole("button", { name: "编辑 Mock JSON Schema" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Mock JSON Schema" }), { target: { value: '{"type":"string","const":"stale local edit"}' } });
  const remote = structuredClone(withExample);
  remote.revision = 4;
  remote.data.collections[0].requests[0].examples[0].mock_schema = '{"type":"string","const":"remote"}';
  await act(async () => client.setQueryData(["shared-workspace", "member", "owner-id", "w", "editor", 0], remote));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  if (dirty) fireEvent.click(screen.getByRole("button", { name: "使用服务器版本" }));
  fireEvent.click(screen.getByRole("button", { name: "编辑 Mock JSON Schema" }));
  expect(screen.getByRole("textbox", { name: "Mock JSON Schema" })).toHaveProperty("value", remote.data.collections[0].requests[0].examples[0].mock_schema);
});

it.each(["linked", "data"] as const)("preserves an existing %s source until explicit inline selection", async (source) => {
  const first = workspace.data.collections[0].requests[0];
  const reference = source === "linked" ? { mock_schema_link: { specification_id: "spec", pointer: "/schema" } } : { mock_data: { dataset_id: "dataset", view_id: "view", shape: "rows" as const, parameters: [] } };
  const withExample: Workspace = { ...workspace, data: { ...workspace.data, collections: [{ ...workspace.data.collections[0], requests: [{ ...first, examples: [{ id: "source-example", name: "Saved source", status: 200, headers: [], body: "retained static", ...reference }] }] }] } };
  vi.mocked(api).mockImplementation((path, method, body) => path.endsWith("/mock-calls") ? Promise.resolve([]) : Promise.resolve(method === "PUT" ? { ...withExample, revision: 4, data: (body as { data: Workspace["data"] }).data } : withExample));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "Mock 预览" }));
  expect(screen.getByLabelText("Mock 正文来源")).toHaveProperty("value", source);
  fireEvent.change(screen.getByLabelText("示例名称"), { target: { value: "Renamed" } });
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", expect.anything()));
  const saved = vi.mocked(api).mock.calls.find(([path, method]) => path === base && method === "PUT")?.[2] as { data: Workspace["data"] };
  expect(saved.data.collections[0].requests[0].examples[0]).toMatchObject(reference);
  await waitFor(() => expect(screen.getByRole("button", { name: "保存共享工作区" })).toHaveProperty("disabled", true));
  fireEvent.change(screen.getByLabelText("Mock 正文来源"), { target: { value: "inline" } });
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(vi.mocked(api).mock.calls.filter(([, method]) => method === "PUT")).toHaveLength(2));
  const sent = vi.mocked(api).mock.calls.filter(([, method]) => method === "PUT")[1][2] as { data: Workspace["data"] };
  const example = sent.data.collections[0].requests[0].examples[0];
  expect(example.mock_schema).toBeTruthy();
  expect(example.mock_data).toBeUndefined();
  expect(example.mock_schema_link).toBeUndefined();
  expect(example.body).toBe("retained static");
});

it("saves shared Mock scripts and match conditions without replacing other example fields", async () => {
  const first = workspace.data.collections[0].requests[0];
  const example = { id: "behavior", name: "Behavior", status: 202, headers: [], body: "static body", mock_schema: '{"type":"object"}', fault_rate_percent: 7, mock_match: { priority: 1, query: [{ id: "q", key: "mode", value: "old", enabled: true }], headers: [{ id: "h", key: "X-Mode", value: "old", enabled: true }], body: "" } };
  const saved: Workspace = { ...workspace, data: { ...workspace.data, script_mode: "v2", collections: [{ ...workspace.data.collections[0], requests: [{ ...first, method: "POST", url: "https://example.test/items/{id}", examples: [example] }] }] } };
  vi.mocked(api).mockImplementation((path, method, body) => path.endsWith("/mock-calls") ? Promise.resolve([]) : Promise.resolve(method === "PUT" ? { ...saved, revision: 4, data: (body as { data: Workspace["data"] }).data } : saved));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "Mock 预览" }));
  expect(screen.getByText(/POST \/api\/owners\/owner-id\/workspaces\/w\/mocks\/items\/\{id\}/)).toBeTruthy();
  fireEvent.change(screen.getByLabelText("匹配优先级"), { target: { value: "9" } });
  fireEvent.change(screen.getByLabelText("参数值"), { target: { value: "fast" } });
  fireEvent.change(screen.getByLabelText("值"), { target: { value: "yes" } });
  fireEvent.change(screen.getByLabelText("精确匹配正文"), { target: { value: "hello" } });
  const script = "pm.mock.setBody('member script')";
  fireEvent.change(screen.getByRole("textbox", { name: /Mock 实现脚本/ }), { target: { value: script } });
  expect(screen.getByRole("button", { name: "预览已保存 Mock 示例" })).toHaveProperty("disabled", true);
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", expect.objectContaining({ expected_revision: 3 })));
  const sent = vi.mocked(api).mock.calls.find(([path, method]) => path === base && method === "PUT")?.[2] as { data: Workspace["data"] };
  expect(sent.data.collections[0].requests[0].examples[0]).toMatchObject({ mock_script: script, body: "static body", mock_schema: example.mock_schema, fault_rate_percent: 7, mock_match: { priority: 9, body: "hello", query: [{ value: "fast" }], headers: [{ value: "yes" }] } });
});

it("keeps Mock script editing unavailable in a v1 shared workspace", async () => {
  const first = workspace.data.collections[0].requests[0];
  const saved: Workspace = { ...workspace, data: { ...workspace.data, script_mode: "v1", collections: [{ ...workspace.data.collections[0], requests: [{ ...first, examples: [{ id: "e", name: "Static", status: 200, body: "static", headers: [] }] }] }] } };
  vi.mocked(api).mockImplementation((path) => Promise.resolve(path.endsWith("/mock-calls") ? [] : saved));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "Mock 预览" }));
  expect(screen.queryByRole("textbox", { name: /Mock 实现脚本/ })).toBeNull();
  expect(screen.getByText("在工作区设置中启用 v2 脚本模式后可编辑动态 Mock。")).toBeTruthy();
});

it("adds and removes a shared response example within the unsaved draft", async () => {
  vi.mocked(api).mockImplementation((path) => Promise.resolve(path.endsWith("/mock-calls") ? [] : workspace));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "Mock 预览" }));
  expect(screen.getByText("所选请求没有响应示例")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "新增响应示例" }));
  expect(screen.getByLabelText("示例名称")).toBeTruthy();
  expect(screen.getByRole("button", { name: "保存共享工作区" })).toHaveProperty("disabled", false);
  fireEvent.click(screen.getByRole("button", { name: "删除示例" }));
  expect(screen.getByText("所选请求没有响应示例")).toBeTruthy();
  expect(screen.getByRole("button", { name: "保存共享工作区" })).toHaveProperty("disabled", true);
});

it("keeps a shared Mock draft visible through a remote revision conflict", async () => {
  const first = workspace.data.collections[0].requests[0];
  const example = { id: "example-a", name: "Success", status: 200, headers: [], body: "original" };
  const withExample: Workspace = { ...workspace, data: { ...workspace.data, collections: [{ ...workspace.data.collections[0], requests: [{ ...first, examples: [example] }] }] } };
  vi.mocked(api).mockImplementation((path, method, body) => {
    if (path.endsWith("/mock-calls")) return Promise.resolve([]);
    if (method === "PUT") return Promise.resolve({ ...withExample, revision: 5, data: (body as { data: Workspace["data"] }).data });
    return Promise.resolve(withExample);
  });
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "Mock 预览" }));
  fireEvent.change(screen.getByLabelText("响应正文"), { target: { value: "member draft" } });
  const remote = { ...withExample, revision: 4, data: { ...withExample.data, collections: [{ ...withExample.data.collections[0], requests: [{ ...first, examples: [{ ...example, body: "owner edit" }] }] }] } };
  await act(async () => client.setQueryData(["shared-workspace", "member", "owner-id", "w", "editor", 0], remote));
  expect(await screen.findByText("共享工作区已被其他成员更新。选择要保留的版本。")).toBeTruthy();
  expect((screen.getByLabelText("响应正文") as HTMLTextAreaElement).value).toBe("member draft");
  expect(screen.getByRole("button", { name: "保存共享工作区" })).toHaveProperty("disabled", true);
  fireEvent.click(screen.getByRole("button", { name: "保留我的修改" }));
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", expect.objectContaining({ expected_revision: 4 })));
  const saved = vi.mocked(api).mock.calls.find(([path, method]) => path === base && method === "PUT")?.[2] as { data: Workspace["data"] };
  expect(saved.data.collections[0].requests[0].examples[0].body).toBe("member draft");
});

it("blocks saving and previewing an incomplete Mock status", async () => {
  const first = workspace.data.collections[0].requests[0];
  const withExample: Workspace = { ...workspace, data: { ...workspace.data, collections: [{ ...workspace.data.collections[0], requests: [{ ...first, examples: [{ id: "example-a", name: "Success", status: 200, headers: [], body: "old" }] }] }] } };
  vi.mocked(api).mockImplementation((path) => Promise.resolve(path.endsWith("/mock-calls") ? [] : withExample));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "Mock 预览" }));
  fireEvent.change(screen.getByLabelText("响应正文"), { target: { value: "edited" } });
  fireEvent.change(screen.getByLabelText("状态码"), { target: { value: "60" } });
  expect(screen.getByText("状态码必须为 200 到 599。")).toBeTruthy();
  expect(screen.getByRole("button", { name: "保存共享工作区" })).toHaveProperty("disabled", true);
  expect(screen.getByRole("button", { name: "预览已保存 Mock 示例" })).toHaveProperty("disabled", true);
  fireEvent.blur(screen.getByLabelText("状态码"));
  expect((screen.getByLabelText("状态码") as HTMLInputElement).value).toBe("200");
  expect(screen.getByRole("button", { name: "保存共享工作区" })).toHaveProperty("disabled", false);
});

it("resets an incomplete status when another request uses the same example ID", async () => {
  const first = workspace.data.collections[0].requests[0];
  const example = { id: "same-example", name: "First", status: 200, headers: [], body: "first" };
  const second = { ...first, id: "second-request", name: "Second", examples: [{ ...example, name: "Second" }] };
  const withTwo: Workspace = { ...workspace, data: { ...workspace.data, collections: [{ ...workspace.data.collections[0], requests: [{ ...first, examples: [example] }, second] }] } };
  vi.mocked(api).mockImplementation((path) => Promise.resolve(path.endsWith("/mock-calls") ? [] : withTwo));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "Mock 预览" }));
  fireEvent.change(screen.getByLabelText("状态码"), { target: { value: "60" } });
  fireEvent.change(screen.getByLabelText("请求"), { target: { value: second.id } });
  expect((screen.getByLabelText("状态码") as HTMLInputElement).value).toBe("200");
  expect(screen.queryByText("状态码必须为 200 到 599。")).toBeNull();
});

it("clears old and delayed Mock previews when a workspace name edit is saved", async () => {
  const first = workspace.data.collections[0].requests[0];
  const example = { id: "example-a", name: "Success", status: 200, headers: [], body: "saved" };
  const withExample: Workspace = { ...workspace, data: { ...workspace.data, collections: [{ ...workspace.data.collections[0], requests: [{ ...first, examples: [example] }] }] } };
  let revision = 3;
  vi.mocked(api).mockImplementation((path, method, body) => {
    if (path.endsWith("/mock-calls")) return Promise.resolve([]);
    if (method === "PUT") return Promise.resolve({ ...withExample, revision: ++revision, name: (body as { name: string }).name });
    return Promise.resolve(withExample);
  });
  let finish!: (value: { status: number; body: string }) => void;
  vi.mocked(previewSharedMock).mockResolvedValueOnce({ status: 200, body: "old-preview" }).mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "Mock 预览" }));
  fireEvent.click(screen.getByRole("button", { name: "预览已保存 Mock 示例" }));
  expect((await screen.findByLabelText("Mock 响应正文") as HTMLTextAreaElement).value).toBe("old-preview");
  fireEvent.click(screen.getByRole("button", { name: "资源" }));
  fireEvent.change(screen.getByLabelText("工作区名称"), { target: { value: "First rename" } });
  fireEvent.click(screen.getByRole("button", { name: "Mock 预览" }));
  expect(screen.queryByLabelText("Mock 响应正文")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "预览已保存 Mock 示例" })).toHaveProperty("disabled", false));
  fireEvent.click(screen.getByRole("button", { name: "预览已保存 Mock 示例" }));
  await waitFor(() => expect(previewSharedMock).toHaveBeenCalledTimes(2));
  fireEvent.click(screen.getByRole("button", { name: "资源" }));
  fireEvent.change(screen.getByLabelText("工作区名称"), { target: { value: "Second rename" } });
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", expect.objectContaining({ name: "Second rename", expected_revision: 4 })));
  await act(async () => finish({ status: 200, body: "delayed-old-preview" }));
  fireEvent.click(screen.getByRole("button", { name: "Mock 预览" }));
  expect(screen.queryByLabelText("Mock 响应正文")).toBeNull();
});

it("drops a delayed Mock preview when the selected example changes", async () => {
  const first = workspace.data.collections[0].requests[0];
  const examples = [{ id: "example-a", name: "First", status: 200, headers: [], body: "first" }, { id: "example-b", name: "Second", status: 202, headers: [], body: "second" }];
  const withExamples: Workspace = { ...workspace, data: { ...workspace.data, collections: [{ ...workspace.data.collections[0], requests: [{ ...first, examples }] }] } };
  vi.mocked(api).mockImplementation((path) => Promise.resolve(path.endsWith("/mock-calls") ? [] : withExamples));
  let finish!: (value: { status: number; body: string }) => void;
  vi.mocked(previewSharedMock).mockImplementation((path) => path.endsWith("example-a") ? new Promise((resolve) => { finish = resolve; }) : Promise.resolve({ status: 202, body: "second" }));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "Mock 预览" }));
  fireEvent.click(await screen.findByRole("button", { name: "预览已保存 Mock 示例" }));
  await waitFor(() => expect(previewSharedMock).toHaveBeenCalledTimes(1));
  fireEvent.change(screen.getByLabelText("响应示例"), { target: { value: "example-b" } });
  fireEvent.click(screen.getByRole("button", { name: "预览已保存 Mock 示例" }));
  expect((await screen.findByLabelText("Mock 响应正文") as HTMLTextAreaElement).value).toBe("second");
  await act(async () => finish({ status: 200, body: "old-private-response" }));
  expect((screen.getByLabelText("Mock 响应正文") as HTMLTextAreaElement).value).toBe("second");
});

it("withholds an in-flight Mock preview after the owner updates the workspace", async () => {
  const first = workspace.data.collections[0].requests[0];
  const example = { id: "example-a", name: "First", status: 200, headers: [], body: "first" };
  const withExample: Workspace = { ...workspace, data: { ...workspace.data, collections: [{ ...workspace.data.collections[0], requests: [{ ...first, examples: [example] }] }] } };
  vi.mocked(api).mockImplementation((path) => Promise.resolve(path.endsWith("/mock-calls") ? [] : withExample));
  let finish!: (value: { status: number; body: string }) => void;
  vi.mocked(previewSharedMock).mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "Mock 预览" }));
  fireEvent.click(await screen.findByRole("button", { name: "预览已保存 Mock 示例" }));
  await waitFor(() => expect(previewSharedMock).toHaveBeenCalledTimes(1));
  await act(async () => client.setQueryData(["shared-workspace", "member", "owner-id", "w", "editor", 0], { ...withExample, revision: 4 }));
  await act(async () => finish({ status: 200, body: "stale-response" }));
  expect(screen.queryByLabelText("Mock 响应正文")).toBeNull();
});

it("saves editor changes against the canonical owner revision", async () => {
  vi.mocked(api).mockImplementation((_path, method) => Promise.resolve(method === "PUT" ? { ...workspace, name: "Renamed", revision: 4 } : workspace));
  render(view("editor"));
  const name = await screen.findByLabelText("工作区名称");
  fireEvent.change(name, { target: { value: "Renamed" } });
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", { name: "Renamed", data: workspace.data, expected_revision: 3 }));
});

it("saves shared HTTP query, headers, JSON body and auth in one canonical revision", async () => {
  vi.mocked(api).mockImplementation((_path, method, body) => Promise.resolve(method === "PUT" ? { ...workspace, revision: 4, data: (body as { data: Workspace["data"] }).data } : workspace));
  render(view("editor"));
  await screen.findByRole("tab", { name: /参数/ });
  fireEvent.click(screen.getByRole("button", { name: "添加一行" }));
  fireEvent.mouseDown(screen.getByRole("tab", { name: /请求头/ }), { button: 0 });
  fireEvent.click(screen.getByRole("button", { name: "添加一行" }));
  fireEvent.mouseDown(screen.getByRole("tab", { name: /请求体/ }), { button: 0 });
  fireEvent.change(screen.getByLabelText("请求体格式"), { target: { value: "json" } });
  fireEvent.change(screen.getByLabelText("请求正文"), { target: { value: '{"hello":"team"}' } });
  fireEvent.mouseDown(screen.getByRole("tab", { name: /鉴权/ }), { button: 0 });
  fireEvent.change(screen.getByLabelText("鉴权类型"), { target: { value: "bearer" } });
  fireEvent.change(screen.getByLabelText("Token"), { target: { value: "member-secret" } });
  expect(screen.queryByRole("button", { name: "发送已保存请求" })).toBeNull();
  expect(screen.getByRole("button", { name: "发送草稿请求" })).toHaveProperty("disabled", false);
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", expect.objectContaining({ expected_revision: 3 })));
  const saved = vi.mocked(api).mock.calls.find(([path, method]) => path === base && method === "PUT")?.[2] as { data: Workspace["data"] };
  expect(saved.data.collections[0].requests[0]).toMatchObject({ query: [{ key: "X-Test", value: "yes" }], headers: [{ key: "X-Test", value: "yes" }], body_kind: "json", body: '{"hello":"team"}', auth: { kind: "bearer", token: "member-secret" } });
  expect(screen.getByRole("tab", { name: /鉴权/ }).getAttribute("aria-selected")).toBe("true");
});

it("does not retain a binary descriptor when switching a shared request to text", async () => {
  const first = workspace.data.collections[0].requests[0];
  const withBinary: Workspace = { ...workspace, data: { ...workspace.data, collections: [{ ...workspace.data.collections[0], requests: [{ ...first, body_kind: "binary", body: '{"file_name":"private.bin","mime":"application/octet-stream","base64":"YQ=="}' }] }] } };
  vi.mocked(api).mockImplementation((_path, method, body) => Promise.resolve(method === "PUT" ? { ...withBinary, revision: 4, data: (body as { data: Workspace["data"] }).data } : withBinary));
  render(view("editor"));
  fireEvent.mouseDown(await screen.findByRole("tab", { name: /请求体/ }), { button: 0 });
  fireEvent.change(screen.getByLabelText("请求体格式"), { target: { value: "text" } });
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", expect.objectContaining({ expected_revision: 3 })));
  const saved = vi.mocked(api).mock.calls.find(([path, method]) => path === base && method === "PUT")?.[2] as { data: Workspace["data"] };
  expect(saved.data.collections[0].requests[0]).toMatchObject({ body_kind: "text", body: "" });
});

it("saves shared HTTP assertions, scripts and network settings through CAS", async () => {
  vi.mocked(api).mockImplementation((_path, method, body) => Promise.resolve(method === "PUT" ? { ...workspace, revision: 4, data: (body as { data: Workspace["data"] }).data } : workspace));
  render(view("editor"));
  fireEvent.mouseDown(await screen.findByRole("tab", { name: /断言/ }), { button: 0 });
  fireEvent.change(screen.getByLabelText("断言名称"), { target: { value: "Shared status" } });
  fireEvent.mouseDown(screen.getByRole("tab", { name: /脚本/ }), { button: 0 });
  fireEvent.change(screen.getByLabelText("请求前 JavaScript 脚本"), { target: { value: "pm.test('shared', () => {});" } });
  fireEvent.mouseDown(screen.getByRole("tab", { name: /网络设置/ }), { button: 0 });
  fireEvent.click(screen.getByText("请求网络设置"));
  fireEvent.change(screen.getByLabelText("连接超时 (ms)"), { target: { value: "2500" } });
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", expect.objectContaining({ expected_revision: 3 })));
  const saved = vi.mocked(api).mock.calls.find(([path, method]) => path === base && method === "PUT")?.[2] as { data: Workspace["data"] };
  expect(saved.data.collections[0].requests[0]).toMatchObject({ assertions: [{ name: "Shared status" }], pre_request_script: "pm.test('shared', () => {});", network: { connect_timeout_ms: 2500 } });
});

for (const kind of ["oauth1", "oauth2"] as const) {
  it(`keeps ${kind} token management out of the shared member editor`, async () => {
    const first = workspace.data.collections[0].requests[0];
    const withOAuth: Workspace = { ...workspace, data: { ...workspace.data, collections: [{ ...workspace.data.collections[0], requests: [{ ...first, auth: { ...first.auth, kind } }] }] } };
    vi.mocked(api).mockResolvedValue(withOAuth);
    render(view("editor"));
    fireEvent.mouseDown(await screen.findByRole("tab", { name: /鉴权/ }), { button: 0 });
    expect(screen.getByText("共享请求的私有 Token 由工作区所有者管理。")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Token.*管理|Manage.*Token/ })).toBeNull();
  });
}

it("keeps edits made while a shared save is pending", async () => {
  let finish!: (value: Workspace) => void;
  vi.mocked(api).mockImplementation((_path, method) => method === "PUT" ? new Promise((resolve) => { finish = resolve; }) : Promise.resolve(workspace));
  render(view("editor"));
  const name = await screen.findByLabelText("工作区名称") as HTMLInputElement;
  fireEvent.change(name, { target: { value: "First edit" } });
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", expect.objectContaining({ name: "First edit", expected_revision: 3 })));
  fireEvent.change(name, { target: { value: "Later edit" } });
  await act(async () => finish({ ...workspace, name: "First edit", revision: 4 }));
  expect(name.value).toBe("Later edit");
  expect(screen.getByRole("button", { name: "保存共享工作区" })).toHaveProperty("disabled", false);
});

it("executes only the selected saved request through its scoped route", async () => {
  const requestId = workspace.data.collections[0].requests[0].id;
  vi.mocked(api).mockImplementation((_path, method) => Promise.resolve(method === "POST" ? { status: 200, status_text: "OK", elapsed_ms: 4, body: "shared-reply" } : workspace));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "发送已保存请求" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(`${base}/requests/${encodeURIComponent(requestId)}/execute`, "POST", {}));
  expect((await screen.findByLabelText("响应正文预览") as HTMLTextAreaElement).value).toBe("shared-reply");
});

it("executes an edited HTTP draft without saving it", async () => {
  const first = workspace.data.collections[0].requests[0];
  vi.mocked(api).mockImplementation((_path, method) => Promise.resolve(method === "POST" ? { status: 200, status_text: "OK", elapsed_ms: 3, body: "draft-wire" } : workspace));
  render(view("editor"));
  fireEvent.change(await screen.findByLabelText("请求地址"), { target: { value: "https://draft.example/" } });
  fireEvent.click(screen.getByRole("button", { name: "发送草稿请求" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(`${base}/execute-draft`, "POST", expect.objectContaining({ collection_id: workspace.data.collections[0].id, expected_workspace_revision: 3, request: expect.objectContaining({ id: first.id, url: "https://draft.example/" }) })));
  expect(screen.getByText(/草稿请求执行结果: 200 OK/)).toBeTruthy();
  expect((screen.getByLabelText("响应正文预览") as HTMLTextAreaElement).value).toBe("draft-wire");
  expect(vi.mocked(api).mock.calls.some(([, method]) => method === "PUT")).toBe(false);
});

it("runs a new unsaved HTTP request in its collection and then saves it", async () => {
  vi.mocked(api).mockImplementation((_path, method, body) => Promise.resolve(method === "POST" ? { status: 200, status_text: "OK", elapsed_ms: 3, body: "new-draft" } : method === "PUT" ? { ...workspace, revision: 4, data: (body as { data: Workspace["data"] }).data } : workspace));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "新增 HTTP 请求" }));
  fireEvent.change(screen.getByLabelText("请求名称"), { target: { value: "New draft" } });
  fireEvent.change(screen.getByLabelText("请求地址"), { target: { value: "https://new.example/" } });
  fireEvent.click(screen.getByRole("button", { name: "发送草稿请求" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(`${base}/execute-draft`, "POST", expect.objectContaining({ collection_id: workspace.data.collections[0].id, expected_workspace_revision: 3, request: expect.objectContaining({ name: "New draft", url: "https://new.example/" }) })));
  const draft = vi.mocked(api).mock.calls.find(([path, method]) => path === `${base}/execute-draft` && method === "POST")?.[2] as { request: { id: string } };
  expect(draft.request.id).not.toBe(workspace.data.collections[0].requests[0].id);
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", expect.objectContaining({ expected_revision: 3 })));
  const saved = vi.mocked(api).mock.calls.find(([path, method]) => path === base && method === "PUT")?.[2] as { data: Workspace["data"] };
  expect(saved.data.collections[0].requests.some((request) => request.id === draft.request.id && request.name === "New draft")).toBe(true);
});

it("stops marking a newly saved draft dirty after canonical server defaults are returned", async () => {
  vi.mocked(api).mockImplementation((_path, method, body) => {
    if (method === "PUT") {
      const data = structuredClone((body as { data: Workspace["data"] }).data);
      const created = data.collections[0].requests.at(-1)!;
      created.protocol = { kind: "http" };
      created.pre_request_script = "";
      created.post_response_script = "";
      return Promise.resolve({ ...workspace, revision: 4, data });
    }
    return Promise.resolve(workspace);
  });
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "新增 HTTP 请求" }));
  fireEvent.change(screen.getByLabelText("请求地址"), { target: { value: "https://new.example/" } });
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", expect.anything()));
  await waitFor(() => expect(screen.getByRole("button", { name: "保存共享工作区" })).toHaveProperty("disabled", true));
  expect(screen.queryByRole("button", { name: "发送草稿请求" })).toBeNull();
  expect(screen.getByRole("button", { name: "发送已保存请求" })).toBeTruthy();
});

it("withholds a delayed draft result after the owner advances the workspace revision", async () => {
  let finish!: (value: unknown) => void;
  vi.mocked(api).mockImplementation((_path, method) => method === "POST" ? new Promise((resolve) => { finish = resolve; }) : Promise.resolve(workspace));
  render(view("editor"));
  fireEvent.change(await screen.findByLabelText("请求地址"), { target: { value: "https://draft.example/" } });
  fireEvent.click(screen.getByRole("button", { name: "发送草稿请求" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(`${base}/execute-draft`, "POST", expect.anything()));
  const remote = { ...workspace, revision: 4 };
  await act(async () => client.setQueryData(["shared-workspace", "member", "owner-id", "w", "editor", 0], remote));
  expect(await screen.findByText("共享工作区已被其他成员更新。选择要保留的版本。")).toBeTruthy();
  await act(async () => finish({ status: 200, status_text: "OK", elapsed_ms: 3, body: "private-old-draft" }));
  expect(screen.queryByLabelText("响应正文预览")).toBeNull();
  expect(screen.getByRole("button", { name: "发送草稿请求" })).toHaveProperty("disabled", true);
});

it("does not show a previous request's delayed response after selection changes", async () => {
  const first = workspace.data.collections[0].requests[0];
  const second = { ...first, id: "second-request", name: "Second request" };
  const withTwo: Workspace = { ...workspace, data: { ...workspace.data, collections: [{ ...workspace.data.collections[0], requests: [first, second] }] } };
  let finishFirst!: (value: unknown) => void;
  vi.mocked(api).mockImplementation((path, method) => {
    if (method === "POST" && path.endsWith(`/${encodeURIComponent(first.id)}/execute`)) return new Promise((resolve) => { finishFirst = resolve; });
    if (method === "POST") return Promise.resolve({ status: 200, status_text: "OK", elapsed_ms: 2, body: "second-response" });
    return Promise.resolve(withTwo);
  });
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "发送已保存请求" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(`${base}/requests/${encodeURIComponent(first.id)}/execute`, "POST", {}));
  fireEvent.change(screen.getByLabelText("请求"), { target: { value: second.id } });
  fireEvent.click(screen.getByRole("button", { name: "发送已保存请求" }));
  expect((await screen.findByLabelText("响应正文预览") as HTMLTextAreaElement).value).toBe("second-response");
  await act(async () => finishFirst({ status: 200, status_text: "OK", elapsed_ms: 9, body: "first-private-response" }));
  expect((screen.getByLabelText("响应正文预览") as HTMLTextAreaElement).value).toBe("second-response");
});

it("withholds a delayed response after editing the selected request", async () => {
  let finish!: (value: unknown) => void;
  vi.mocked(api).mockImplementation((_path, method) => method === "POST" ? new Promise((resolve) => { finish = resolve; }) : Promise.resolve(workspace));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "发送已保存请求" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(expect.stringContaining("/execute"), "POST", {}));
  fireEvent.change(screen.getByLabelText("请求地址"), { target: { value: "https://example.test/changed" } });
  await act(async () => finish({ status: 200, status_text: "OK", elapsed_ms: 5, body: "old-private-response" }));
  expect(screen.queryByLabelText("响应正文预览")).toBeNull();
  expect(screen.getByRole("button", { name: "保存共享工作区" })).toHaveProperty("disabled", false);
});

it("does not show a previous collection's delayed run result after selection changes", async () => {
  const first = workspace.data.collections[0];
  const second = { ...first, id: "second-collection", name: "Second collection", requests: [] };
  const withTwo: Workspace = { ...workspace, data: { ...workspace.data, collections: [first, second] } };
  let finish!: (value: unknown) => void;
  vi.mocked(api).mockImplementation((_path, method) => method === "POST" ? new Promise((resolve) => { finish = resolve; }) : Promise.resolve(withTwo));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "运行集合" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(`${base}/run`, "POST", expect.objectContaining({ collection_id: first.id })));
  fireEvent.change(screen.getByLabelText("集合"), { target: { value: second.id } });
  await act(async () => finish({ passed: 1, failed: 0, cancelled: false }));
  expect(screen.queryByText(/通过: 1/)).toBeNull();
});

it("withholds an old response when another editor replaces the saved request at the same ID", async () => {
  let finish!: (value: unknown) => void;
  vi.mocked(api).mockImplementation((_path, method) => method === "POST" ? new Promise((resolve) => { finish = resolve; }) : Promise.resolve(workspace));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "发送已保存请求" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(expect.stringContaining("/execute"), "POST", {}));
  const changed: Workspace = { ...workspace, revision: 4, data: { ...workspace.data, collections: workspace.data.collections.map((collection) => ({ ...collection, requests: collection.requests.map((request) => ({ ...request, url: "https://example.test/new-owner-url" })) })) } };
  await act(async () => client.setQueryData(["shared-workspace", "member", "owner-id", "w", "editor", 0], changed));
  await waitFor(() => expect((screen.getByLabelText("请求地址") as HTMLInputElement).value).toBe("https://example.test/new-owner-url"));
  await act(async () => finish({ status: 200, status_text: "OK", elapsed_ms: 5, body: "old-private-response" }));
  expect(screen.queryByLabelText("响应正文预览")).toBeNull();
});

it("selects a shared dataset view without replacing body, script or traffic settings", async () => {
  const first = workspace.data.collections[0].requests[0];
  const shared: Workspace = { ...workspace, data: { ...workspace.data, script_mode: "v2", datasets: [{ id: "data-a", name: "Rows", description: "", secret: true, source: { format: "json", source: '[{"id":"a"}]' }, views: [{ id: "all", name: "All", sql: "SELECT id FROM data" }] }], collections: [{ ...workspace.data.collections[0], requests: [{ ...first, examples: [{ id: "data-example", name: "Data", status: 201, headers: [{ id: "ct", key: "Content-Type", value: "text/plain", enabled: true }], body: "keep static", mock_script: "console.log('keep')", delay_ms: 12, mock_schema: '{"type":"object"}', mock_name_inference: true }] }] }] } };
  vi.mocked(api).mockImplementation((path, method, body) => {
    if (path.endsWith("/mock-calls")) return Promise.resolve([]);
    if (method !== "PUT") return Promise.resolve(shared);
    const data = structuredClone((body as { data: Workspace["data"] }).data);
    Reflect.deleteProperty(data.collections[0].requests[0].examples[0].mock_data!, "parameters");
    return Promise.resolve({ ...shared, revision: 4, data });
  });
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "Mock 预览" }));
  fireEvent.change(screen.getByLabelText("Mock 正文来源"), { target: { value: "data" } });
  expect(await screen.findByRole("combobox", { name: "只读数据视图" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", expect.anything()));
  const sent = vi.mocked(api).mock.calls.find(([path, method]) => path === base && method === "PUT")?.[2] as { data: Workspace["data"] };
  expect(sent.data.collections[0].requests[0].examples[0]).toMatchObject({ body: "keep static", mock_script: "console.log('keep')", delay_ms: 12, mock_data: { dataset_id: "data-a", view_id: "all", shape: "rows", parameters: [] }, headers: [{ value: "application/json" }] });
  expect(sent.data.collections[0].requests[0].examples[0].mock_schema).toBeUndefined();
  expect(sent.data.collections[0].requests[0].examples[0].mock_name_inference).toBeUndefined();
  await waitFor(() => expect(screen.getByRole("button", { name: "预览已保存 Mock 示例" })).toHaveProperty("disabled", false));
  expect(vi.mocked(api).mock.calls.every(([path]) => path === base || path.startsWith(`${base}/`))).toBe(true);
});

it("edits shared data parameters through CAS and directs parameterized previews to the member route", async () => {
  const first = workspace.data.collections[0].requests[0];
  const shared: Workspace = { ...workspace, data: { ...workspace.data, script_mode: "v2", datasets: [{ id: "data-a", name: "Rows", description: "", secret: false, views: [{ id: "by-id", name: "By ID", sql: "SELECT id FROM data WHERE id = ?" }] }], collections: [{ ...workspace.data.collections[0], requests: [{ ...first, url: "https://example.test/items/{id}", examples: [{ id: "data-example", name: "Data", status: 200, headers: [], body: "retained", mock_data: { dataset_id: "data-a", view_id: "by-id", shape: "first", parameters: [] } }] }] }] } };
  vi.mocked(api).mockImplementation((path, method, body) => Promise.resolve(path.endsWith("/mock-calls") ? [] : method === "PUT" ? { ...shared, revision: 4, data: (body as { data: Workspace["data"] }).data } : shared));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "Mock 预览" }));
  fireEvent.click(screen.getByRole("button", { name: "添加视图参数" }));
  fireEvent.change(screen.getByRole("textbox", { name: "参数名 1" }), { target: { value: "id" } });
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "保存共享工作区" })).toHaveProperty("disabled", true));
  expect(screen.getByRole("button", { name: "预览已保存 Mock 示例" })).toHaveProperty("disabled", true);
  expect(screen.getByText("带请求参数的数据绑定请通过匹配路由调用。")).toBeTruthy();
  const sent = vi.mocked(api).mock.calls.find(([path, method]) => path === base && method === "PUT")?.[2] as { data: Workspace["data"] };
  expect(sent.data.collections[0].requests[0].examples[0].mock_data).toMatchObject({ shape: "first", parameters: [{ source: "query", name: "id" }] });
  expect(previewSharedMock).not.toHaveBeenCalled();
});

it.each([false, true])("preserves a shared binding and warns when its %s dataset/view is missing", async (missingView) => {
  const first = workspace.data.collections[0].requests[0];
  const shared: Workspace = { ...workspace, data: { ...workspace.data, datasets: missingView ? [{ id: "missing", name: "Rows", description: "", secret: false, views: [] }] : [], collections: [{ ...workspace.data.collections[0], requests: [{ ...first, examples: [{ id: "e", name: "Data", status: 200, headers: [], body: "retained", mock_data: { dataset_id: "missing", view_id: "gone", shape: "rows", parameters: [] } }] }] }] } };
  vi.mocked(api).mockImplementation((path) => Promise.resolve(path.endsWith("/mock-calls") ? [] : shared));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "Mock 预览" }));
  expect(screen.getByRole("alert").textContent).toContain("所选数据集或只读视图已不可用。");
  expect(screen.getByLabelText("Mock 正文来源")).toHaveProperty("value", "data");
  expect(screen.getByRole("button", { name: "保存共享工作区" })).toHaveProperty("disabled", true);
});

it("does not offer a new shared data binding in v1 mode", async () => {
  const first = workspace.data.collections[0].requests[0];
  const shared: Workspace = { ...workspace, data: { ...workspace.data, script_mode: "v1", datasets: [{ id: "d", name: "Rows", description: "", secret: false, views: [{ id: "all", name: "All", sql: "SELECT 1" }] }], collections: [{ ...workspace.data.collections[0], requests: [{ ...first, examples: [{ id: "e", name: "Static", status: 200, headers: [], body: "retained" }] }] }] } };
  vi.mocked(api).mockImplementation((path) => Promise.resolve(path.endsWith("/mock-calls") ? [] : shared));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "Mock 预览" }));
  expect(screen.queryByRole("option", { name: "已保存的数据绑定" })).toBeNull();
  expect(screen.queryByRole("combobox", { name: "只读数据视图" })).toBeNull();
});

it("does not offer an HTTP-only data binding for a shared WebSocket request", async () => {
  const first = workspace.data.collections[0].requests[0];
  const shared: Workspace = { ...workspace, data: { ...workspace.data, script_mode: "v2", datasets: [{ id: "d", name: "Rows", description: "", secret: false, views: [{ id: "all", name: "All", sql: "SELECT 1" }] }], collections: [{ ...workspace.data.collections[0], requests: [{ ...first, protocol: { kind: "websocket" }, url: "ws://example.test/items", examples: [{ id: "e", name: "Static", status: 200, headers: [], body: "retained" }] }] }] } };
  vi.mocked(api).mockImplementation((path) => Promise.resolve(path.endsWith("/mock-calls") ? [] : shared));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "Mock 预览" }));
  expect(screen.queryByRole("option", { name: "已保存的数据绑定" })).toBeNull();
});

const linkedPointer = "#/paths/~1items/post/responses/201/content/application~1json/schema";
function linkedMockWorkspace(): Workspace {
  const first = workspace.data.collections[0].requests[0];
  return { ...workspace, data: { ...workspace.data, specifications: [{ id: "spec", name: "Responses", kind: "openapi", dialect: "3.1.0", source: JSON.stringify({ openapi: "3.1.0", paths: { "/items": { post: { responses: { "201": { content: { "application/json": { schema: { type: "object", properties: { id: { const: 7 } }, required: ["id"] } } } } } } } } }) }], collections: [{ ...workspace.data.collections[0], requests: [{ ...first, method: "POST", url: "https://example.test/items", specification_id: "spec", examples: [{ id: "linked-e", name: "Link", status: 200, headers: [{ id: "ct", key: "Content-Type", value: "text/plain", enabled: true }], body: "keep static", delay_ms: 12, mock_script: "console.log('keep')", mock_schema: '{"type":"object"}' }] }] }] } };
}

it("validates a shared OpenAPI response before saving its link, status and media header", async () => {
  const shared = linkedMockWorkspace();
  vi.mocked(api).mockImplementation((path, method, body) => Promise.resolve(path.endsWith("/mock-calls") ? [] : path.endsWith("/mock-schema/validate") ? { method: "POST", status: 201, media_type: "application/json" } : method === "PUT" ? { ...shared, revision: 4, data: (body as { data: Workspace["data"] }).data } : shared));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "Mock 预览" }));
  expect(screen.getByRole("option", { name: "POST /items · 201 application/json" })).toBeTruthy();
  fireEvent.change(screen.getByLabelText("Mock 正文来源"), { target: { value: linkedPointer } });
  await waitFor(() => expect(api).toHaveBeenCalledWith(`${base}/mock-schema/validate`, "POST", { request_id: shared.data.collections[0].requests[0].id, specification_id: "spec", pointer: linkedPointer, expected_revision: 3 }));
  await waitFor(() => expect(screen.getByLabelText("状态码")).toHaveProperty("value", "201"));
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", expect.anything()));
  const sent = vi.mocked(api).mock.calls.find(([path, method]) => path === base && method === "PUT")?.[2] as { data: Workspace["data"] };
  expect(sent.data.collections[0].requests[0].examples[0]).toMatchObject({ status: 201, headers: [{ value: "application/json" }], body: "keep static", delay_ms: 12, mock_script: "console.log('keep')", mock_schema_link: { specification_id: "spec", pointer: linkedPointer } });
  expect(sent.data.collections[0].requests[0].examples[0].mock_schema).toBeUndefined();
  expect(vi.mocked(api).mock.calls.every(([path]) => path === base || path.startsWith(`${base}/`))).toBe(true);
});

it("retains the source when shared linked Schema validation fails or requires a saved draft", async () => {
  const shared = linkedMockWorkspace();
  vi.mocked(api).mockImplementation((path) => path.endsWith("/mock-schema/validate") ? Promise.reject(new Error("Unsupported Schema")) : Promise.resolve(path.endsWith("/mock-calls") ? [] : shared));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "Mock 预览" }));
  fireEvent.change(screen.getByLabelText("Mock 正文来源"), { target: { value: linkedPointer } });
  expect(await screen.findByText(/Unsupported Schema/)).toBeTruthy();
  expect(screen.getByLabelText("Mock 正文来源")).toHaveProperty("value", "inline");
  fireEvent.change(screen.getByLabelText("示例名称"), { target: { value: "Unsaved" } });
  fireEvent.change(screen.getByLabelText("Mock 正文来源"), { target: { value: linkedPointer } });
  expect(await screen.findByText("先保存工作区，再选择关联响应。")).toBeTruthy();
  expect(vi.mocked(api).mock.calls.filter(([path]) => path.endsWith("/mock-schema/validate"))).toHaveLength(1);
});

it.each(["edit", "source", "status", "selection", "role", "account", "revision"] as const)("discards delayed linked Schema metadata after %s changes", async (boundary) => {
  const shared = linkedMockWorkspace();
  shared.data.collections[0].requests[0].examples.push({ ...shared.data.collections[0].requests[0].examples[0], id: "other-e", name: "Other" });
  let finish!: (value: unknown) => void;
  vi.mocked(api).mockImplementation((path) => path.endsWith("/mock-schema/validate") ? new Promise((resolve) => { finish = resolve; }) : Promise.resolve(path.endsWith("/mock-calls") ? [] : shared));
  const { rerender } = render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "Mock 预览" }));
  fireEvent.change(screen.getByLabelText("Mock 正文来源"), { target: { value: linkedPointer } });
  await waitFor(() => expect(finish).toBeTypeOf("function"));
  expect(screen.getByText("正在校验关联响应…")).toBeTruthy();
  if (boundary === "edit") fireEvent.change(screen.getByLabelText("示例名称"), { target: { value: "Later edit" } });
  if (boundary === "source") fireEvent.change(screen.getByLabelText("Mock 正文来源"), { target: { value: "static" } });
  if (boundary === "status") fireEvent.change(screen.getByLabelText("状态码"), { target: { value: "1" } });
  if (boundary === "selection") fireEvent.change(screen.getByLabelText("响应示例"), { target: { value: "other-e" } });
  if (boundary === "role") rerender(view("viewer"));
  if (boundary === "account") rerender(view("editor", "other-member"));
  if (boundary === "revision") await act(async () => client.setQueryData(["shared-workspace", "member", "owner-id", "w", "editor", 0], { ...shared, revision: 4 }));
  await act(async () => finish({ method: "POST", status: 201, media_type: "application/json" }));
  if (boundary === "role") expect(screen.queryByRole("button", { name: "Mock 预览" })).toBeNull();
  else {
    if (boundary === "account") fireEvent.click(await screen.findByRole("button", { name: "Mock 预览" }));
    expect(screen.getByLabelText("状态码")).toHaveProperty("value", boundary === "status" ? "1" : "200");
    expect(screen.getByLabelText("Mock 正文来源")).toHaveProperty("value", boundary === "source" ? "static" : "inline");
  }
  expect(screen.queryByText("正在校验关联响应…")).toBeNull();
  expect(vi.mocked(api).mock.calls.some(([, method]) => method === "PUT")).toBe(false);
});

it("shows a stale linked response without silently replacing it", async () => {
  const shared = linkedMockWorkspace();
  shared.data.collections[0].requests[0].examples[0].mock_schema = undefined;
  shared.data.collections[0].requests[0].examples[0].mock_schema_link = { specification_id: "spec", pointer: "#/removed" };
  vi.mocked(api).mockImplementation((path) => Promise.resolve(path.endsWith("/mock-calls") ? [] : shared));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "Mock 预览" }));
  expect(screen.getByRole("alert").textContent).toContain("关联的响应与当前规范不匹配；请重新选择。");
  expect(screen.getByLabelText("Mock 正文来源")).toHaveProperty("value", "linked");
  expect(screen.getByRole("button", { name: "保存共享工作区" })).toHaveProperty("disabled", true);
});

it("creates the first collection in an empty shared workspace only after Apply and canonical Save", async () => {
  const shared: Workspace = { ...workspace, data: { ...workspace.data, collections: [] } };
  vi.mocked(api).mockImplementation((_path, method, body) => Promise.resolve(method === "PUT" ? { ...shared, revision: 4, data: (body as { data: Workspace["data"] }).data } : shared));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "新建集合" }));
  fireEvent.change(screen.getByRole("textbox", { name: "名称" }), { target: { value: "First collection" } });
  expect(vi.mocked(api).mock.calls.some(([, method]) => method === "PUT")).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "应用" }));
  expect(await screen.findByRole("heading", { name: "First collection" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "新增 HTTP 请求" }));
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", expect.anything()));
  const sent = vi.mocked(api).mock.calls.find(([path, method]) => path === base && method === "PUT")?.[2] as { data: Workspace["data"] };
  expect(sent.data.collections).toHaveLength(1);
  expect(sent.data.collections[0]).toMatchObject({ name: "First collection", requests: [{ protocol: { kind: "http" } }] });
});

it("edits shared collection metadata while preserving saved requests", async () => {
  vi.mocked(api).mockImplementation((_path, method, body) => Promise.resolve(method === "PUT" ? { ...workspace, revision: 4, data: (body as { data: Workspace["data"] }).data } : workspace));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "集合与目录设置" }));
  fireEvent.change(screen.getByRole("textbox", { name: "名称" }), { target: { value: "Shared settings" } });
  fireEvent.mouseDown(screen.getByRole("tab", { name: /脚本/ }), { button: 0, ctrlKey: false });
  fireEvent.change(await screen.findByRole("textbox", { name: /前置脚本/ }), { target: { value: "pm.variables.set('collection', 'yes')" } });
  fireEvent.click(screen.getByRole("button", { name: "应用" }));
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", expect.anything()));
  const sent = vi.mocked(api).mock.calls.find(([path, method]) => path === base && method === "PUT")?.[2] as { data: Workspace["data"] };
  expect(sent.data.collections[0].name).toBe("Shared settings");
  expect(sent.data.collections[0].pre_request_script).toBe("pm.variables.set('collection', 'yes')");
  expect(sent.data.collections[0].requests).toEqual(workspace.data.collections[0].requests);
});

it("creates a shared child folder under the current collection and discards Cancel", async () => {
  vi.mocked(api).mockImplementation((_path, method, body) => Promise.resolve(method === "PUT" ? { ...workspace, revision: 4, data: (body as { data: Workspace["data"] }).data } : workspace));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "新建目录" }));
  fireEvent.change(screen.getByRole("textbox", { name: "名称" }), { target: { value: "Cancelled child" } });
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(screen.queryByRole("option", { name: "Cancelled child" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "新建目录" }));
  fireEvent.change(screen.getByRole("textbox", { name: "名称" }), { target: { value: "Child folder" } });
  expect(screen.getByLabelText("父目录")).toHaveProperty("value", `collection:${workspace.data.collections[0].id}`);
  fireEvent.click(screen.getByRole("button", { name: "应用" }));
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", expect.anything()));
  const sent = vi.mocked(api).mock.calls.find(([path, method]) => path === base && method === "PUT")?.[2] as { data: Workspace["data"] };
  expect(sent.data.collections[1]).toMatchObject({ name: "Child folder", parent_id: workspace.data.collections[0].id, requests: [] });
  expect(sent.data.collections[0]).toEqual(workspace.data.collections[0]);
});

it.each(["account", "credential", "role", "saved", "conflict"] as const)("closes unapplied shared collection settings after %s changes", async (boundary) => {
  const { rerender } = render(view("editor"));
  await screen.findByRole("button", { name: "集合与目录设置" });
  if (boundary === "conflict") fireEvent.change(screen.getByLabelText("工作区名称"), { target: { value: "Unsaved workspace" } });
  fireEvent.click(screen.getByRole("button", { name: "集合与目录设置" }));
  fireEvent.change(screen.getByRole("textbox", { name: "名称" }), { target: { value: "Unapplied collection" } });
  if (boundary === "account") rerender(view("editor", "another-member"));
  if (boundary === "role") rerender(view("viewer"));
  if (boundary === "credential") { vi.mocked(token).mockReturnValue("new-credential"); rerender(view("editor")); }
  if (boundary === "saved" || boundary === "conflict") await act(async () => client.setQueryData(["shared-workspace", "member", "owner-id", "w", "editor", 0], { ...workspace, revision: 4 }));
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(screen.queryByDisplayValue("Unapplied collection")).toBeNull();
  expect(vi.mocked(api).mock.calls.some(([, method]) => method === "PUT")).toBe(false);
});

it("keeps collection creation and settings absent for a viewer", async () => {
  render(view("viewer"));
  await screen.findByLabelText("集合");
  expect(screen.queryByRole("button", { name: "新建集合" })).toBeNull();
  expect(screen.queryByRole("button", { name: "新建目录" })).toBeNull();
  expect(screen.queryByRole("button", { name: "集合与目录设置" })).toBeNull();
});

it("duplicates a shared request with a new identity and saves the same authored contents", async () => {
  vi.mocked(api).mockImplementation((_path, method, body) => Promise.resolve(method === "PUT" ? { ...workspace, revision: 4, data: (body as { data: Workspace["data"] }).data } : workspace));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "复制请求" }));
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", expect.anything()));
  const sent = vi.mocked(api).mock.calls.find(([path, method]) => path === base && method === "PUT")?.[2] as { data: Workspace["data"] };
  const copy = sent.data.collections[0].requests.at(-1)!;
  expect(copy.id).not.toBe(workspace.data.collections[0].requests[0].id);
  expect({ ...copy, id: "", name: "" }).toEqual({ ...workspace.data.collections[0].requests[0], id: "", name: "" });
});

it("stages shared request deletion only after confirmation and persists only on Save", async () => {
  vi.mocked(api).mockImplementation((_path, method, body) => Promise.resolve(method === "PUT" ? { ...workspace, revision: 4, data: (body as { data: Workspace["data"] }).data } : workspace));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "删除请求" }));
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(screen.getByLabelText("请求名称")).toHaveProperty("value", workspace.data.collections[0].requests[0].name);
  fireEvent.click(screen.getByRole("button", { name: "删除请求" }));
  fireEvent.click(screen.getByRole("button", { name: "确认删除" }));
  expect(vi.mocked(api).mock.calls.some(([, method]) => method === "PUT")).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", expect.anything()));
  const sent = vi.mocked(api).mock.calls.find(([path, method]) => path === base && method === "PUT")?.[2] as { data: Workspace["data"] };
  expect(sent.data.collections[0].requests.map((request) => request.id)).toEqual(workspace.data.collections[0].requests.slice(1).map((request) => request.id));
});

it("moves a shared request to a selected collection while preserving its identity and contents", async () => {
  const shared = structuredClone(workspace);
  shared.data.collections.push({ id: "target", name: "Destination", description: "", requests: [] });
  vi.mocked(api).mockImplementation((_path, method, body) => Promise.resolve(method === "PUT" ? { ...shared, revision: 4, data: (body as { data: Workspace["data"] }).data } : shared));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "移动请求" }));
  fireEvent.change(screen.getByLabelText("目标集合"), { target: { value: "target" } });
  fireEvent.click(screen.getByRole("button", { name: "确认移动" }));
  expect(await screen.findByRole("heading", { name: "Destination" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", expect.anything()));
  const sent = vi.mocked(api).mock.calls.find(([path, method]) => path === base && method === "PUT")?.[2] as { data: Workspace["data"] };
  expect(sent.data.collections[1].requests).toEqual([workspace.data.collections[0].requests[0]]);
  expect(sent.data.collections[0].requests).toEqual(workspace.data.collections[0].requests.slice(1));
});

it("reviews subtree deletion and reparents its documents only on canonical Save", async () => {
  const shared = structuredClone(workspace); const rootId = shared.data.collections[0].id;
  shared.data.collections.push({ id: "child", name: "Child", parent_id: rootId, description: "", requests: [] }, { id: "other", name: "Other", description: "", requests: [] });
  shared.data.documents = [{ id: "doc", name: "Guide", source: "retained", collection_id: "child" }];
  vi.mocked(api).mockImplementation((_path, method, body) => Promise.resolve(method === "PUT" ? { ...shared, revision: 4, data: (body as { data: Workspace["data"] }).data } : shared));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "删除集合" }));
  expect(screen.getByText("删除范围：2 个集合，2 个请求；1 篇文档移至工作区根目录。")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "确认删除" }));
  expect(vi.mocked(api).mock.calls.some(([, method]) => method === "PUT")).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", expect.anything()));
  const sent = vi.mocked(api).mock.calls.find(([path, method]) => path === base && method === "PUT")?.[2] as { data: Workspace["data"] };
  expect(sent.data.collections.map((collection) => collection.id)).toEqual(["other"]);
  expect(sent.data.documents?.[0]).toMatchObject({ source: "retained", collection_id: null });
});

it("names structural dependencies and keeps referenced resources unchanged", async () => {
  const shared = structuredClone(workspace);
  shared.data.scenarios = [{ id: "suite", name: "Uses this request", description: "", collection_id: shared.data.collections[0].id, steps: [{ id: "step", request_id: shared.data.collections[0].requests[0].id, name: "Step", group: "", enabled: true }] }];
  vi.mocked(api).mockResolvedValue(shared);
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "删除请求" }));
  expect(screen.getByText(/Uses this request/)).toBeTruthy();
  expect(screen.getByRole("button", { name: "确认删除" })).toHaveProperty("disabled", true);
  expect(vi.mocked(api).mock.calls.some(([, method]) => method === "PUT")).toBe(false);
});

it.each(["account", "credential", "role", "selection", "revision", "conflict", "draft"] as const)("discards a pending resource deletion after %s changes", async (boundary) => {
  const { rerender } = render(view("editor"));
  await screen.findByRole("button", { name: "删除请求" });
  if (boundary === "conflict") fireEvent.change(screen.getByLabelText("工作区名称"), { target: { value: "Dirty" } });
  fireEvent.click(screen.getByRole("button", { name: "删除请求" }));
  expect(screen.getByRole("alertdialog")).toBeTruthy();
  if (boundary === "account") rerender(view("editor", "another-account"));
  if (boundary === "credential") { vi.mocked(token).mockReturnValue("other-credential"); rerender(view("editor")); }
  if (boundary === "role") rerender(view("viewer"));
  if (boundary === "selection") fireEvent.change(screen.getByLabelText("请求"), { target: { value: workspace.data.collections[0].requests[1].id } });
  if (boundary === "draft") fireEvent.change(screen.getByLabelText("工作区名称"), { target: { value: "Later edit" } });
  if (boundary === "revision" || boundary === "conflict") await act(async () => client.setQueryData(["shared-workspace", "member", "owner-id", "w", "editor", 0], { ...workspace, revision: 4 }));
  await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
  expect(vi.mocked(api).mock.calls.some(([, method]) => method === "PUT")).toBe(false);
});

it("keeps all shared resource actions absent for viewers", async () => {
  render(view("viewer"));
  await screen.findByLabelText("请求");
  for (const name of ["复制请求", "移动请求", "删除请求", "删除集合"]) expect(screen.queryByRole("button", { name })).toBeNull();
});

it("applies a shared scenario to the canonical draft and preserves notification source IDs", async () => {
  const shared = structuredClone(workspace);
  shared.data.scenarios = [{ id: "suite", name: "Original suite", description: "", collection_id: shared.data.collections[0].id, notification_ids: ["saved-notification"], steps: [{ id: "step", request_id: shared.data.collections[0].requests[0].id, name: "Step", group: "", enabled: true }] }];
  vi.mocked(api).mockImplementation((_path, method, body) => Promise.resolve(method === "PUT" ? { ...shared, revision: 4, data: (body as { data: Workspace["data"] }).data } : shared));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "工作区测试场景" }));
  fireEvent.click(screen.getByRole("button", { name: "编辑场景" }));
  fireEvent.change(screen.getByRole("textbox", { name: "场景名称" }), { target: { value: "Shared suite" } });
  fireEvent.change(screen.getByLabelText("场景请求"), { target: { value: shared.data.collections[0].requests[1].id } });
  fireEvent.click(screen.getByRole("button", { name: "应用场景修改" }));
  expect(vi.mocked(api).mock.calls.some(([, method]) => method === "PUT")).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base, "PUT", expect.anything()));
  const sent = vi.mocked(api).mock.calls.find(([path, method]) => path === base && method === "PUT")?.[2] as { data: Workspace["data"] };
  expect(sent.data.scenarios?.[0]).toMatchObject({ id: "suite", name: "Shared suite", notification_ids: ["saved-notification"], steps: [{ request_id: shared.data.collections[0].requests[1].id }] });
  expect(vi.mocked(api).mock.calls.every(([path]) => path === base || path.startsWith(`${base}/`))).toBe(true);
});

it("runs a saved shared scenario through the creator-bound member run endpoint", async () => {
  const shared = structuredClone(workspace);
  shared.data.scenarios = [{ id: "suite", name: "Saved flow", description: "", collection_id: shared.data.collections[0].id, steps: [{ id: "step", request_id: shared.data.collections[0].requests[0].id, name: "Step", group: "", enabled: true }] }];
  vi.mocked(api).mockImplementation((path) => Promise.resolve(path.endsWith("/run") ? { passed: 1, failed: 0, cancelled: false } : shared));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "工作区测试场景" }));
  fireEvent.click(screen.getByRole("button", { name: "运行已保存场景" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(`${base}/run`, "POST", expect.objectContaining({ collection_id: shared.data.collections[0].id, scenario_id: "suite", job_id: expect.any(String) })));
});

it("updates a scenario reference and can then delete the original request in one shared draft", async () => {
  const shared = structuredClone(workspace);const root=shared.data.collections[0];
  shared.data.scenarios = [{ id: "suite", name: "References", description: "", collection_id: root.id, steps: [{ id: "step", request_id: root.requests[0].id, name: "Step", group: "", enabled: true }] }];
  vi.mocked(api).mockImplementation((_path, method, body) => Promise.resolve(method === "PUT" ? { ...shared, revision: 4, data: (body as { data: Workspace["data"] }).data } : shared));
  render(view("editor"));
  fireEvent.click(await screen.findByRole("button", { name: "工作区测试场景" }));
  fireEvent.click(screen.getByRole("button", { name: "编辑场景" }));
  expect(screen.getByRole("button", { name: "运行已保存场景" })).toHaveProperty("disabled", true);
  fireEvent.change(screen.getByLabelText("场景请求"), { target: { value: root.requests[1].id } });
  fireEvent.click(screen.getByRole("button", { name: "应用场景修改" }));
  expect(screen.getByRole("button", { name: "运行已保存场景" })).toHaveProperty("disabled", true);
  fireEvent.click(screen.getByRole("button", { name: "资源" }));
  fireEvent.click(screen.getByRole("button", { name: "删除请求" }));
  expect(screen.getByRole("button", { name: "确认删除" })).toHaveProperty("disabled", false);
  fireEvent.click(screen.getByRole("button", { name: "确认删除" }));
  fireEvent.click(screen.getByRole("button", { name: "保存共享工作区" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(base,"PUT",expect.anything()));
  const sent=vi.mocked(api).mock.calls.find(([path,method])=>path===base&&method==="PUT")?.[2] as {data:Workspace["data"]};
  expect(sent.data.scenarios?.[0].steps[0].request_id).toBe(root.requests[1].id);
  expect(sent.data.collections[0].requests.map((request)=>request.id)).toEqual([root.requests[1].id]);
});

it("cancels scenario removal, then stages a confirmed removal without deleting requests", async () => {
  const shared=structuredClone(workspace);const root=shared.data.collections[0];
  shared.data.scenarios=[{id:"suite",name:"Keep requests",description:"",collection_id:root.id,steps:[{id:"step",request_id:root.requests[0].id,name:"Step",group:"",enabled:true}]}];
  vi.mocked(api).mockImplementation((_path,method,body)=>Promise.resolve(method==="PUT"?{...shared,revision:4,data:(body as {data:Workspace["data"]}).data}:shared));
  render(view("editor"));fireEvent.click(await screen.findByRole("button",{name:"工作区测试场景"}));
  fireEvent.click(screen.getByRole("button",{name:"删除场景"}));fireEvent.click(screen.getByRole("button",{name:"取消"}));
  expect(screen.getByText("Keep requests")).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"删除场景"}));fireEvent.click(screen.getByRole("button",{name:"确认删除"}));
  expect(vi.mocked(api).mock.calls.some(([,method])=>method==="PUT")).toBe(false);
  fireEvent.click(screen.getByRole("button",{name:"保存共享工作区"}));await waitFor(()=>expect(api).toHaveBeenCalledWith(base,"PUT",expect.anything()));
  const sent=vi.mocked(api).mock.calls.find(([path,method])=>path===base&&method==="PUT")?.[2] as {data:Workspace["data"]};
  expect(sent.data.scenarios).toEqual([]);expect(sent.data.collections[0].requests).toEqual(root.requests);
});

it("creates only subtree request references and discards unapplied scenario edits on Cancel", async () => {
  const shared=structuredClone(workspace);const root=shared.data.collections[0];
  shared.data.collections.push({...root,id:"child",parent_id:root.id,name:"Child",requests:[{...root.requests[0],id:"nested",name:"Nested"}]},{...root,id:"other",name:"Other",requests:[{...root.requests[0],id:"excluded",name:"Excluded"}]});
  vi.mocked(api).mockResolvedValue(shared);render(view("editor"));fireEvent.click(await screen.findByRole("button",{name:"工作区测试场景"}));
  fireEvent.click(screen.getByRole("button",{name:"新建场景"}));fireEvent.change(screen.getByRole("textbox",{name:"场景名称"}),{target:{value:"Unapplied"}});
  fireEvent.click(screen.getByRole("button",{name:"添加请求步骤"}));
  expect(screen.getByRole("option",{name:"Child / Nested"})).toBeTruthy();expect(screen.queryByRole("option",{name:"Other / Excluded"})).toBeNull();
  fireEvent.click(screen.getByRole("button",{name:"取消"}));expect(screen.queryByDisplayValue("Unapplied")).toBeNull();expect(screen.getByRole("button",{name:"保存共享工作区"})).toHaveProperty("disabled",true);
});

it.each(["account","credential","role","root","revision","conflict"] as const)("discards unapplied scenario edits after %s changes",async(boundary)=>{
  const shared=structuredClone(workspace);shared.data.collections.push({id:"other",name:"Other",description:"",requests:[]});
  vi.mocked(api).mockResolvedValue(shared);const {rerender}=render(view("editor"));
  await screen.findByRole("button",{name:"工作区测试场景"});
  if(boundary==="conflict")fireEvent.change(screen.getByLabelText("工作区名称"),{target:{value:"Dirty"}});
  fireEvent.click(screen.getByRole("button",{name:"工作区测试场景"}));fireEvent.click(screen.getByRole("button",{name:"新建场景"}));
  fireEvent.change(screen.getByRole("textbox",{name:"场景名称"}),{target:{value:"Stale scenario"}});
  if(boundary==="account")rerender(view("editor","another-member"));
  if(boundary==="credential"){vi.mocked(token).mockReturnValue("other-token");rerender(view("editor"));}
  if(boundary==="role")rerender(view("viewer"));
  if(boundary==="root")fireEvent.change(screen.getByLabelText("集合"),{target:{value:"other"}});
  if(boundary==="revision"||boundary==="conflict")await act(async()=>client.setQueryData(["shared-workspace","member","owner-id","w","editor",0],{...shared,revision:4}));
  await waitFor(()=>expect(screen.queryByDisplayValue("Stale scenario")).toBeNull());
  expect(vi.mocked(api).mock.calls.some(([,method])=>method==="PUT")).toBe(false);
});

it("stops a running saved scenario using the original member job ID",async()=>{
  const shared=structuredClone(workspace);const root=shared.data.collections[0];shared.data.scenarios=[{id:"suite",name:"Stop flow",description:"",collection_id:root.id,steps:[{id:"step",request_id:root.requests[0].id,name:"Step",group:"",enabled:true}]}];
  let finish!:(value:unknown)=>void;vi.mocked(api).mockImplementation((path)=>path.endsWith("/run")?new Promise((resolve)=>{finish=resolve;}):Promise.resolve(path.endsWith("/run/cancel")?{cancelled:true}:shared));
  render(view("editor"));fireEvent.click(await screen.findByRole("button",{name:"工作区测试场景"}));fireEvent.click(screen.getByRole("button",{name:"运行已保存场景"}));
  await waitFor(()=>expect(finish).toBeTypeOf("function"));expect(screen.getByRole("button",{name:"新建场景"})).toHaveProperty("disabled",true);
  const input=vi.mocked(api).mock.calls.find(([path])=>path.endsWith("/run"))?.[2] as {job_id:string};
  fireEvent.click(screen.getByRole("button",{name:"停止集合运行"}));await waitFor(()=>expect(api).toHaveBeenCalledWith(`${base}/run/cancel`,"POST",{job_id:input.job_id}));
  await act(async()=>finish({passed:0,failed:0,cancelled:true}));expect(await screen.findByText(/已取消/)).toBeTruthy();
});

it("hides scenario editing and execution from viewers",async()=>{
  render(view("viewer"));await screen.findByLabelText("请求");expect(screen.queryByRole("button",{name:"工作区测试场景"})).toBeNull();expect(screen.queryByRole("button",{name:"运行已保存场景"})).toBeNull();
});

it.each(["edit", "selection", "delete", "save", "remote"] as const)("keeps the active creator job stoppable after %s invalidates its inline result", async (change) => {
  const shared=structuredClone(workspace);shared.data.collections.push({id:"other",name:"Other",description:"",requests:[{...shared.data.collections[0].requests[0],id:"other-request"}]});
  let finish!:(value:unknown)=>void;
  vi.mocked(api).mockImplementation((path,method,body)=>{
    if(path===`${base}/run`)return new Promise(resolve=>{finish=resolve;});
    if(path===`${base}/run/cancel`)return Promise.resolve({cancelled:true});
    if(method==="PUT")return Promise.resolve({...shared,revision:4,name:(body as {name:string}).name,data:(body as {data:Workspace["data"]}).data});
    return Promise.resolve(shared);
  });
  render(view("editor"));fireEvent.click(await screen.findByRole("button",{name:"运行集合"}));
  await waitFor(()=>expect(finish).toBeTypeOf("function"));
  const job=vi.mocked(api).mock.calls.find(([path])=>path===`${base}/run`)?.[2] as {job_id:string};
  if(change==="edit"||change==="save")fireEvent.change(screen.getByLabelText("工作区名称"),{target:{value:"Edited while running"}});
  if(change==="save"){fireEvent.click(screen.getByRole("button",{name:"保存共享工作区"}));await waitFor(()=>expect(api).toHaveBeenCalledWith(base,"PUT",expect.anything()));}
  if(change==="selection")fireEvent.change(screen.getByLabelText("集合"),{target:{value:"other"}});
  if(change==="delete"){fireEvent.click(screen.getByRole("button",{name:"删除集合"}));fireEvent.click(screen.getByRole("button",{name:"确认删除"}));}
  if(change==="remote")await act(async()=>client.setQueryData(["shared-workspace","member","owner-id","w","editor",0],{...shared,revision:4}));
  expect(screen.getByRole("button",{name:"停止集合运行"})).toBeTruthy();
  expect(screen.getByRole("button",{name:"运行集合"})).toHaveProperty("disabled",true);
  fireEvent.click(screen.getByRole("button",{name:"停止集合运行"}));
  await waitFor(()=>expect(api).toHaveBeenCalledWith(`${base}/run/cancel`,"POST",{job_id:job.job_id}));
  expect(screen.getByRole("button",{name:"停止集合运行"})).toBeTruthy();
  await act(async()=>finish({passed:17,failed:0,cancelled:true}));
  await waitFor(()=>expect(screen.queryByRole("button",{name:"停止集合运行"})).toBeNull());
  expect(screen.queryByText(/17/)).toBeNull();
  expect(vi.mocked(api).mock.calls.filter(([path])=>path===`${base}/run`)).toHaveLength(1);
});

it("keeps one active Stop across every shared page and after deleting the last collection",async()=>{
  let finish!:(value:unknown)=>void;
  vi.mocked(api).mockImplementation((path)=>path===`${base}/run`?new Promise(resolve=>{finish=resolve;}):Promise.resolve(path.endsWith("/history")||path.endsWith("/mock-calls")?[]:path.endsWith("/reports")?{items:[],next_cursor:null}:workspace));
  render(view("editor"));fireEvent.click(await screen.findByRole("button",{name:"运行集合"}));await waitFor(()=>expect(finish).toBeTypeOf("function"));
  for(const name of ["文档","环境变量","工作区测试场景","执行历史","测试报告","Mock 预览","资源"]){fireEvent.click(screen.getByRole("button",{name}));expect(screen.getAllByRole("button",{name:"停止集合运行"})).toHaveLength(1);}
  fireEvent.click(screen.getByRole("button",{name:"删除集合"}));fireEvent.click(screen.getByRole("button",{name:"确认删除"}));
  expect(screen.getByText("此工作区还没有集合")).toBeTruthy();expect(screen.getByRole("button",{name:"停止集合运行"})).toBeTruthy();
  await act(async()=>finish({passed:0,failed:0,cancelled:true}));
});

it.each(["success","failure"] as const)("releases an invalidated job after actual %s and permits a later saved run",async(outcome)=>{
  let finish!:(value:unknown)=>void;let fail!:(error:Error)=>void;let runs=0;
  vi.mocked(api).mockImplementation((path,method,body)=>{
    if(path===`${base}/run`){runs++;return runs===1?new Promise((resolve,reject)=>{finish=resolve;fail=reject;}):Promise.resolve({passed:1,failed:0,cancelled:false});}
    return Promise.resolve(method==="PUT"?{...workspace,revision:4,name:(body as {name:string}).name}:workspace);
  });
  render(view("editor"));fireEvent.click(await screen.findByRole("button",{name:"运行集合"}));await waitFor(()=>expect(finish).toBeTypeOf("function"));
  fireEvent.change(screen.getByLabelText("工作区名称"),{target:{value:"Changed"}});
  await act(async()=>{if(outcome==="success")finish({passed:19,failed:0,cancelled:false});else fail(new Error("old failure"));});
  expect(screen.queryByRole("button",{name:"停止集合运行"})).toBeNull();expect(screen.queryByText(/19|old failure/)).toBeNull();
  fireEvent.click(screen.getByRole("button",{name:"保存共享工作区"}));await waitFor(()=>expect(screen.getByRole("button",{name:"运行集合"})).toHaveProperty("disabled",false));
  fireEvent.click(screen.getByRole("button",{name:"运行集合"}));await waitFor(()=>expect(runs).toBe(2));expect(await screen.findByText(/通过: 1/)).toBeTruthy();
});

it("refreshes saved reports after a same-scope run whose inline result was invalidated",async()=>{
  let finish!:(value:unknown)=>void;let completed=false;
  vi.mocked(api).mockImplementation((path)=>{
    if(path===`${base}/run`)return new Promise(resolve=>{finish=resolve;});
    if(path===`${base}/reports`)return Promise.resolve({items:completed?[{id:"report",collection_name:"Saved after edit",summary:{passed:1,failed:0}}]:[],next_cursor:null});
    return Promise.resolve(workspace);
  });
  render(view("editor"));fireEvent.click(await screen.findByRole("button",{name:"运行集合"}));await waitFor(()=>expect(finish).toBeTypeOf("function"));
  fireEvent.change(screen.getByLabelText("工作区名称"),{target:{value:"Edited"}});fireEvent.click(screen.getByRole("button",{name:"测试报告"}));await screen.findByText("暂无测试报告");
  completed=true;await act(async()=>finish({passed:19,failed:0,cancelled:false}));
  expect(await screen.findByText("Saved after edit")).toBeTruthy();expect(screen.queryByText(/19/)).toBeNull();
});

it.each(["account","credential","role","workspace","owner"] as const)("fences the old job and Stop after the %s scope changes",async(boundary)=>{
  const finishes:((value:unknown)=>void)[]=[];
  vi.mocked(api).mockImplementation((path)=>path.endsWith("/run")?new Promise(resolve=>{finishes.push(resolve);}):Promise.resolve(path.endsWith("/run/cancel")?{cancelled:true}:workspace));
  const {rerender}=render(view("editor"));fireEvent.click(await screen.findByRole("button",{name:"运行集合"}));await waitFor(()=>expect(finishes).toHaveLength(1));
  if(boundary==="account")rerender(view("editor","new-account"));
  if(boundary==="credential"){vi.mocked(token).mockReturnValue("new-token");rerender(view("editor"));}
  if(boundary==="role")rerender(view("viewer"));
  if(boundary==="workspace"||boundary==="owner")rerender(<QueryClientProvider client={client}><Theme><SharedWorkspaceView accountId="member" selected={{...summary("editor"),...(boundary==="workspace"?{workspace_id:"other"}:{owner_id:"other-owner"})}} dark={false} onClose={vi.fn()} onDirtyChange={vi.fn()}/></Theme></QueryClientProvider>);
  await waitFor(()=>expect(screen.queryByRole("button",{name:"停止集合运行"})).toBeNull());
  if(boundary!=="role"){
    fireEvent.click(await screen.findByRole("button",{name:"运行集合"}));await waitFor(()=>expect(finishes).toHaveLength(2));
    await act(async()=>finishes[0]({passed:19,failed:0,cancelled:false}));
    expect(screen.getByRole("button",{name:"停止集合运行"})).toBeTruthy();expect(screen.queryByText(/19/)).toBeNull();
    const calls=vi.mocked(api).mock.calls.filter(([path])=>path.endsWith("/run"));const body=calls[1][2] as {job_id:string};
    fireEvent.click(screen.getByRole("button",{name:"停止集合运行"}));await waitFor(()=>expect(api).toHaveBeenCalledWith(calls[1][0]+"/cancel","POST",{job_id:body.job_id}));
    await act(async()=>finishes[1]({passed:0,failed:0,cancelled:true}));
  }else{await act(async()=>finishes[0]({passed:19,failed:0,cancelled:false}));expect(screen.queryByText(/19/)).toBeNull();}
});

it("does not let a late Stop acknowledgment change a newer job's busy state",async()=>{
  const finishes:((value:unknown)=>void)[]=[];const stops:((value:unknown)=>void)[]=[];
  vi.mocked(api).mockImplementation((path)=>path===`${base}/run`?new Promise(resolve=>{finishes.push(resolve);}):path===`${base}/run/cancel`?new Promise(resolve=>{stops.push(resolve);}):Promise.resolve(workspace));
  render(view("editor"));fireEvent.click(await screen.findByRole("button",{name:"运行集合"}));await waitFor(()=>expect(finishes).toHaveLength(1));
  fireEvent.click(screen.getByRole("button",{name:"停止集合运行"}));await waitFor(()=>expect(stops).toHaveLength(1));
  await act(async()=>finishes[0]({passed:0,failed:0,cancelled:true}));fireEvent.click(screen.getByRole("button",{name:"运行集合"}));await waitFor(()=>expect(finishes).toHaveLength(2));
  fireEvent.click(screen.getByRole("button",{name:"停止集合运行"}));await waitFor(()=>expect(stops).toHaveLength(2));
  await act(async()=>stops[0]({cancelled:true}));expect(screen.getByRole("button",{name:"停止集合运行"})).toHaveProperty("disabled",true);
  await act(async()=>stops[1]({cancelled:true}));expect(screen.getByRole("button",{name:"停止集合运行"})).toHaveProperty("disabled",false);
  await act(async()=>finishes[1]({passed:0,failed:0,cancelled:true}));expect(screen.queryByRole("button",{name:"停止集合运行"})).toBeNull();
});

it("creates a shared literal dataset and read-only view through member preview and canonical CAS",async()=>{
  vi.mocked(api).mockImplementation((path,method,body)=>Promise.resolve(path.endsWith("/dataset/preview")?{columns:["id"],rows:[{id:7}]}:method==="PUT"?{...workspace,revision:4,data:(body as {data:Workspace["data"]}).data}:workspace));
  render(view("editor"));fireEvent.click(await screen.findByRole("button",{name:"工作区数据集"}));fireEvent.click(screen.getByRole("button",{name:"新建数据集"}));
  fireEvent.change(screen.getByRole("textbox",{name:"数据集名称"}),{target:{value:"Member rows"}});
  fireEvent.change(screen.getByRole("textbox",{name:"测试数据源"}),{target:{value:'[{"id":7}]'}});
  fireEvent.click(screen.getByRole("button",{name:"添加视图"}));fireEvent.change(screen.getByRole("textbox",{name:"视图名称"}),{target:{value:"All rows"}});fireEvent.change(screen.getByRole("textbox",{name:"视图 SQL"}),{target:{value:"SELECT id FROM data"}});
  fireEvent.click(screen.getByRole("button",{name:"应用数据集修改"}));await waitFor(()=>expect(api).toHaveBeenCalledWith(`${base}/dataset/preview`,"POST",{expected_revision:3,dataset:{format:"json",source:'[{"id":7}]'}}));
  expect(vi.mocked(api).mock.calls.some(([,method])=>method==="PUT")).toBe(false);
  fireEvent.click(screen.getByRole("button",{name:"保存共享工作区"}));await waitFor(()=>expect(api).toHaveBeenCalledWith(base,"PUT",expect.anything()));
  const sent=vi.mocked(api).mock.calls.find(([path,method])=>path===base&&method==="PUT")?.[2] as {data:Workspace["data"]};
  expect(sent.data.datasets?.[0]).toMatchObject({name:"Member rows",secret:true,source:{format:"json",source:'[{"id":7}]'},views:[{name:"All rows",sql:"SELECT id FROM data"}]});
  expect(vi.mocked(api).mock.calls.every(([path])=>path===base||path.startsWith(`${base}/`))).toBe(true);
});

it("stages a private Data query dataset without copying SQL or calling a preview route",async()=>{
  const shared=structuredClone(workspace);shared.data.collections[0].requests[0].protocol={...dataConfig(),sql:"SELECT private_column FROM records"};shared.data.collections[0].requests[0].url="postgresql://member:dataset-query-secret@localhost/test";
  vi.mocked(api).mockImplementation((_path,method,body)=>Promise.resolve(method==="PUT"?{...shared,revision:4,data:(body as {data:Workspace["data"]}).data}:shared));
  render(view("editor"));fireEvent.click(await screen.findByRole("button",{name:"工作区数据集"}));fireEvent.click(screen.getByRole("button",{name:"新建数据集"}));
  fireEvent.change(screen.getByRole("textbox",{name:"数据集名称"}),{target:{value:"Fresh rows"}});fireEvent.change(screen.getByLabelText("数据集来源"),{target:{value:"query"}});
  fireEvent.change(screen.getByLabelText("选择只读 Data 请求"),{target:{value:shared.data.collections[0].requests[0].id}});
  fireEvent.click(screen.getByRole("button",{name:"应用数据集修改"}));fireEvent.click(screen.getByRole("button",{name:"保存共享工作区"}));await waitFor(()=>expect(api).toHaveBeenCalledWith(base,"PUT",expect.anything()));
  const sent=vi.mocked(api).mock.calls.find(([path,method])=>path===base&&method==="PUT")?.[2] as {data:Workspace["data"]};
  expect(sent.data.datasets?.[0]).toMatchObject({secret:true,query:{request_id:shared.data.collections[0].requests[0].id}});expect(sent.data.datasets?.[0].source).toBeUndefined();expect(JSON.stringify(sent.data.datasets)).not.toContain("private_column");
  expect(vi.mocked(api).mock.calls.some(([path])=>path.includes("preview"))).toBe(false);
});

function withBoundDataset():Workspace{
  const shared=structuredClone(workspace);shared.data.script_mode="v2";
  shared.data.datasets=[{id:"dataset",name:"Bound rows",description:"",secret:true,source:{format:"json",source:'[{"id":7}]'},views:[{id:"view",name:"All rows",sql:"SELECT id FROM data"}]}];
  shared.data.collections[0].requests[0].examples=[{id:"example",name:"Bound example",status:200,headers:[],body:"static",mock_data:{dataset_id:"dataset",view_id:"view",shape:"rows",parameters:[]}}];return shared;
}

it("names Mock dependencies before dataset or view removal and retains original source on Cancel",async()=>{
  const shared=withBoundDataset();vi.mocked(api).mockResolvedValue(shared);render(view("editor"));fireEvent.click(await screen.findByRole("button",{name:"工作区数据集"}));
  fireEvent.click(screen.getByRole("button",{name:"删除数据集"}));expect(screen.getByText(/Bound example/)).toBeTruthy();expect(screen.getByRole("button",{name:"确认删除"})).toHaveProperty("disabled",true);
  fireEvent.click(screen.getByRole("button",{name:"取消"}));fireEvent.click(screen.getByRole("button",{name:"编辑数据集"}));fireEvent.click(screen.getByRole("button",{name:"删除视图"}));
  expect(screen.getByRole("button",{name:"应用数据集修改"})).toHaveProperty("disabled",true);expect(screen.getByText(/Bound example/)).toBeTruthy();fireEvent.click(screen.getByRole("button",{name:"取消"}));
  expect(screen.getByRole("button",{name:"保存共享工作区"})).toHaveProperty("disabled",true);expect(vi.mocked(api).mock.calls.some(([,method])=>method==="PUT")).toBe(false);
});

it("can clear a Mock binding then delete its dataset in one canonical draft",async()=>{
  const shared=withBoundDataset();vi.mocked(api).mockImplementation((path,method,body)=>Promise.resolve(path.endsWith("/mock-calls")?[]:method==="PUT"?{...shared,revision:4,data:(body as {data:Workspace["data"]}).data}:shared));
  render(view("editor"));fireEvent.click(await screen.findByRole("button",{name:"Mock 预览"}));fireEvent.change(screen.getByLabelText("Mock 正文来源"),{target:{value:"static"}});
  fireEvent.click(screen.getByRole("button",{name:"工作区数据集"}));fireEvent.click(screen.getByRole("button",{name:"删除数据集"}));fireEvent.click(screen.getByRole("button",{name:"确认删除"}));
  fireEvent.click(screen.getByRole("button",{name:"保存共享工作区"}));await waitFor(()=>expect(api).toHaveBeenCalledWith(base,"PUT",expect.anything()));
  const sent=vi.mocked(api).mock.calls.find(([path,method])=>path===base&&method==="PUT")?.[2] as {data:Workspace["data"]};expect(sent.data.datasets).toEqual([]);expect(sent.data.collections[0].requests[0].examples[0].mock_data).toBeUndefined();expect(sent.data.collections[0].requests[0].examples[0].body).toBe("static");
});

it.each(["edit","account","credential","role","revision","conflict"] as const)("discards a delayed dataset Apply after %s changes",async(boundary)=>{
  let finish!:(value:unknown)=>void;vi.mocked(api).mockImplementation((path)=>path.endsWith("/dataset/preview")?new Promise(resolve=>{finish=resolve;}):Promise.resolve(workspace));
  const {rerender}=render(view("editor"));await screen.findByRole("button",{name:"工作区数据集"});if(boundary==="conflict")fireEvent.change(screen.getByLabelText("工作区名称"),{target:{value:"Dirty"}});
  fireEvent.click(screen.getByRole("button",{name:"工作区数据集"}));fireEvent.click(screen.getByRole("button",{name:"新建数据集"}));fireEvent.change(screen.getByRole("textbox",{name:"数据集名称"}),{target:{value:"Old form"}});fireEvent.change(screen.getByRole("textbox",{name:"测试数据源"}),{target:{value:'[{"id":7}]'}});fireEvent.click(screen.getByRole("button",{name:"应用数据集修改"}));await waitFor(()=>expect(finish).toBeTypeOf("function"));
  if(boundary==="edit")fireEvent.change(screen.getByRole("textbox",{name:"数据集名称"}),{target:{value:"Later form"}});
  if(boundary==="account")rerender(view("editor","other-account"));if(boundary==="credential"){vi.mocked(token).mockReturnValue("new-token");rerender(view("editor"));}if(boundary==="role")rerender(view("viewer"));
  if(boundary==="revision"||boundary==="conflict")await act(async()=>client.setQueryData(["shared-workspace","member","owner-id","w","editor",0],{...workspace,revision:4}));
  await act(async()=>finish({columns:["id"],rows:[{id:7}]}));expect(screen.queryByText("Old form")).toBeNull();expect(vi.mocked(api).mock.calls.some(([,method])=>method==="PUT")).toBe(false);
  if(boundary==="edit")expect(screen.getByRole("textbox",{name:"数据集名称"})).toHaveProperty("value","Later form");
});

it("hides shared dataset data and mutation controls from viewers",async()=>{
  render(view("viewer"));await screen.findByLabelText("请求");expect(screen.queryByRole("button",{name:"工作区数据集"})).toBeNull();expect(screen.queryByRole("button",{name:"新建数据集"})).toBeNull();
});

it("shares saved dataset IDs and iterations between collection and scenario runs", async () => {
  const shared = structuredClone(workspace), collection = shared.data.collections[0];
  shared.data.datasets = [{ id: "private", name: "Private rows", description: "", secret: true, source: { format: "json", source: '[{"value":"private-source"}]' } }];
  shared.data.scenarios = [{ id: "scenario-data", name: "Data scenario", description: "", collection_id: collection.id, steps: [{ id: "step", request_id: collection.requests[0].id, name: "", group: "", enabled: true, repeat: 1 }] }];
  vi.mocked(api).mockImplementation((path) => Promise.resolve(path.endsWith("/run") ? { passed: 1, failed: 0, cancelled: false } : shared));
  render(view("editor"));
  fireEvent.change(await screen.findByLabelText("运行数据集"), { target: { value: "dataset:private" } });
  fireEvent.change(screen.getByLabelText("迭代次数"), { target: { value: "2" } });
  fireEvent.click(screen.getByRole("button", { name: "运行集合" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(`${base}/run`, "POST", expect.objectContaining({ dataset_id: "private", iterations: 2 })));
  await waitFor(() => expect(screen.getByRole("button", { name: "运行集合" })).toHaveProperty("disabled", false));
  fireEvent.click(screen.getByRole("button", { name: "工作区测试场景" }));
  fireEvent.click(await screen.findByRole("button", { name: "运行已保存场景" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(`${base}/run`, "POST", expect.objectContaining({ scenario_id: "scenario-data", dataset_id: "private", iterations: 2 })));
  const runs = vi.mocked(api).mock.calls.filter(([path]) => path === `${base}/run`);
  expect(runs).toHaveLength(2);
  for (const [, , body] of runs) { expect(body).not.toHaveProperty("dataset"); expect(JSON.stringify(body)).not.toContain("private-source"); }
  expect(vi.mocked(api).mock.calls.some(([, method]) => method === "PUT")).toBe(false);
});

it("previews temporary run data through the member route without saving and runs inline CSV", async () => {
  vi.mocked(api).mockImplementation((path) => Promise.resolve(path.endsWith("/dataset/preview") ? { columns: ["value"], rows: [{ value: "alpha" }] } : path.endsWith("/run") ? { passed: 1, failed: 0, cancelled: false } : workspace));
  render(view("editor"));
  fireEvent.change(await screen.findByLabelText("运行数据集"), { target: { value: "temporary" } });
  expect(screen.getByRole("button", { name: "运行集合" })).toHaveProperty("disabled", true);
  fireEvent.change(screen.getByLabelText("数据格式"), { target: { value: "csv" } });
  fireEvent.change(screen.getByLabelText("测试数据源"), { target: { value: "value\nalpha" } });
  fireEvent.click(screen.getByRole("button", { name: "预览数据行" }));
  await screen.findByText("alpha");
  expect(api).toHaveBeenCalledWith(`${base}/dataset/preview`, "POST", { expected_revision: 3, dataset: { format: "csv", source: "value\nalpha" } });
  fireEvent.click(screen.getByRole("button", { name: "运行集合" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(`${base}/run`, "POST", expect.objectContaining({ dataset: { format: "csv", source: "value\nalpha" } })));
  expect(vi.mocked(api).mock.calls.some(([path, method]) => path.startsWith("/api/testing/") || method === "PUT")).toBe(false);
});

it("accepts query run data and blocks a selected dataset deleted by another member", async () => {
  const shared = structuredClone(workspace);
  shared.data.datasets = [{ id: "query", name: "Fresh query", description: "", secret: true, query: { request_id: "query-request" } }];
  vi.mocked(api).mockImplementation((path) => Promise.resolve(path.endsWith("/run") ? { passed: 1, failed: 0, cancelled: false } : shared));
  render(view("editor"));
  fireEvent.change(await screen.findByLabelText("运行数据集"), { target: { value: "dataset:query" } });
  fireEvent.click(screen.getByRole("button", { name: "运行集合" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(`${base}/run`, "POST", expect.objectContaining({ dataset_id: "query" })));
  act(() => client.setQueryData(["shared-workspace", "member", "owner-id", "w", "editor", 0], { ...shared, revision: 4, data: { ...shared.data, datasets: [] } }));
  await screen.findByText("所选数据集已删除或缺少数据源，请重新选择。");
  expect(screen.getByLabelText("运行数据集")).toHaveProperty("value", "dataset:query");
  expect(screen.getByRole("button", { name: "运行集合" })).toHaveProperty("disabled", true);
});

it("retains temporary input across tabs and resets it across account scope", async () => {
  const mounted = render(view("editor"));
  fireEvent.change(await screen.findByLabelText("运行数据集"), { target: { value: "temporary" } });
  fireEvent.change(screen.getByLabelText("测试数据源"), { target: { value: '[{"private":"temporary-secret"}]' } });
  fireEvent.click(screen.getByRole("button", { name: "文档" }));
  fireEvent.click(screen.getByRole("button", { name: "资源" }));
  expect((screen.getByLabelText("测试数据源") as HTMLTextAreaElement).value).toContain("temporary-secret");
  mounted.rerender(view("editor", "different-member"));
  await screen.findByLabelText("运行数据集");
  expect(screen.getByLabelText("运行数据集")).toHaveProperty("value", "none");
  expect(screen.queryByLabelText("测试数据源")).toBeNull();
});

it("locks run settings while preserving the original dataset job and global Stop across tabs", async () => {
  let finish!: (value: unknown) => void;
  const shared = structuredClone(workspace);
  shared.data.datasets = [{ id: "d", name: "Rows", description: "", secret: true, source: { format: "csv", source: "value\nalpha" } }];
  vi.mocked(api).mockImplementation((path) => path === `${base}/run` ? new Promise((resolve) => { finish = resolve; }) : Promise.resolve(shared));
  render(view("editor"));
  fireEvent.change(await screen.findByLabelText("运行数据集"), { target: { value: "dataset:d" } });
  fireEvent.click(screen.getByRole("button", { name: "运行集合" }));
  await waitFor(() => expect(finish).toBeTypeOf("function"));
  expect(screen.getByLabelText("运行数据集")).toHaveProperty("disabled", true);
  expect(screen.getByLabelText("迭代次数")).toHaveProperty("disabled", true);
  fireEvent.click(screen.getByRole("button", { name: "文档" }));
  fireEvent.click(screen.getByRole("button", { name: "停止集合运行" }));
  const body = vi.mocked(api).mock.calls.find(([path]) => path === `${base}/run`)?.[2] as { job_id: string };
  await waitFor(() => expect(api).toHaveBeenCalledWith(`${base}/run/cancel`, "POST", { job_id: body.job_id }));
  act(() => finish({ passed: 1, failed: 0, cancelled: true }));
  await waitFor(() => expect(screen.queryByRole("button", { name: "停止集合运行" })).toBeNull());
  fireEvent.click(screen.getByRole("button", { name: "资源" }));
  expect(screen.getByLabelText("运行数据集")).toHaveProperty("value", "dataset:d");
  expect(screen.getByLabelText("运行数据集")).toHaveProperty("disabled", false);
});

it("does not dispatch run against a newly cached revision before React updates saved data", async () => {
  const shared = structuredClone(workspace);
  shared.data.datasets = [{ id: "d", name: "Rows", description: "", secret: true, source: { format: "csv", source: "value\nalpha" } }];
  vi.mocked(api).mockResolvedValue(shared);
  render(view("editor"));
  fireEvent.change(await screen.findByLabelText("运行数据集"), { target: { value: "dataset:d" } });
  act(() => {
    client.setQueryData(["shared-workspace", "member", "owner-id", "w", "editor", 0], { ...shared, revision: 4, data: { ...shared.data, datasets: [] } });
    fireEvent.click(screen.getByRole("button", { name: "运行集合" }));
  });
  expect(vi.mocked(api).mock.calls.some(([path]) => path === `${base}/run`)).toBe(false);
});
