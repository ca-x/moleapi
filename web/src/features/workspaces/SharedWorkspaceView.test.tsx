// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Theme } from "@radix-ui/themes";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { api, previewSharedMock, token } from "../../shared/api";
import { setLanguage } from "../../shared/i18n";
import { initialData } from "../../shared/model";
import type { SharedWorkspaceSummary, Workspace } from "../../shared/types";
import SharedWorkspaceView from "./SharedWorkspaceView";

vi.mock("../../shared/api", () => ({ api: vi.fn(), previewSharedMock: vi.fn(), token: vi.fn() }));
vi.mock("../../shared/ui", () => ({ Field: ({ label, children }: { label: string; children: React.ReactNode }) => <label>{label}{children}</label>, Choice: ({ label, value, options, onChange }: { label: string; value: string; options: { value: string; label: string }[]; onChange: (value: string) => void }) => <label>{label}<select value={value} onChange={(event) => onChange(event.target.value)}>{options.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</select></label> }));
const base = "/api/owners/owner-id/workspaces/w";
const workspace: Workspace = { id: "w", name: "Team", revision: 3, updated_at: "now", data: initialData() };
const summary = (role: SharedWorkspaceSummary["role"]): SharedWorkspaceSummary => ({ owner_id: "owner-id", owner_username: "owner", workspace_id: "w", workspace_name: "Team", revision: 3, role });
let client: QueryClient;
function view(role: SharedWorkspaceSummary["role"]) { return <QueryClientProvider client={client}><Theme><SharedWorkspaceView accountId="member" selected={summary(role)} onClose={vi.fn()} onDirtyChange={vi.fn()} /></Theme></QueryClientProvider>; }
beforeEach(async () => { await setLanguage("zh-CN"); client = new QueryClient({ defaultOptions: { queries: { retry: false } } }); vi.mocked(token).mockReturnValue("session-member"); vi.mocked(api).mockReset(); vi.mocked(api).mockResolvedValue(workspace); vi.mocked(previewSharedMock).mockReset(); });
afterEach(() => { cleanup(); client.clear(); });

it("keeps viewer resources read-only and hides private execution controls", async () => {
  render(view("viewer"));
  await screen.findByLabelText("请求");
  expect(screen.getByText("此工作区为只读。不能保存、运行请求或查看私有执行记录。")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "保存共享工作区" })).toBeNull();
  expect(screen.queryByRole("button", { name: "执行历史" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Mock 预览" })).toBeNull();
  expect(api).toHaveBeenCalledWith(base);
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
