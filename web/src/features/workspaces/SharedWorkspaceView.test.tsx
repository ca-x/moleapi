// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Theme } from "@radix-ui/themes";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { api, token } from "../../shared/api";
import { setLanguage } from "../../shared/i18n";
import { initialData } from "../../shared/model";
import type { SharedWorkspaceSummary, Workspace } from "../../shared/types";
import SharedWorkspaceView from "./SharedWorkspaceView";

vi.mock("../../shared/api", () => ({ api: vi.fn(), token: vi.fn() }));
vi.mock("../../shared/ui", () => ({ Field: ({ label, children }: { label: string; children: React.ReactNode }) => <label>{label}{children}</label>, Choice: ({ label, value, options, onChange }: { label: string; value: string; options: { value: string; label: string }[]; onChange: (value: string) => void }) => <label>{label}<select value={value} onChange={(event) => onChange(event.target.value)}>{options.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</select></label> }));
const base = "/api/owners/owner-id/workspaces/w";
const workspace: Workspace = { id: "w", name: "Team", revision: 3, updated_at: "now", data: initialData() };
const summary = (role: SharedWorkspaceSummary["role"]): SharedWorkspaceSummary => ({ owner_id: "owner-id", owner_username: "owner", workspace_id: "w", workspace_name: "Team", revision: 3, role });
let client: QueryClient;
function view(role: SharedWorkspaceSummary["role"]) { return <QueryClientProvider client={client}><Theme><SharedWorkspaceView accountId="member" selected={summary(role)} onClose={vi.fn()} onDirtyChange={vi.fn()} /></Theme></QueryClientProvider>; }
beforeEach(async () => { await setLanguage("zh-CN"); client = new QueryClient({ defaultOptions: { queries: { retry: false } } }); vi.mocked(token).mockReturnValue("session-member"); vi.mocked(api).mockReset(); vi.mocked(api).mockResolvedValue(workspace); });
afterEach(() => { cleanup(); client.clear(); });

it("keeps viewer resources read-only and hides private execution controls", async () => {
  render(view("viewer"));
  await screen.findByLabelText("请求");
  expect(screen.getByText("此工作区为只读。不能保存、运行请求或查看私有执行记录。")).toBeTruthy();
  expect(screen.queryByRole("button", { name: "保存共享工作区" })).toBeNull();
  expect(screen.queryByRole("button", { name: "执行历史" })).toBeNull();
  expect(api).toHaveBeenCalledWith(base);
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
