// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { Theme } from "@radix-ui/themes";
import { afterEach, expect, it, vi } from "vitest";
import { api } from "../../shared/api";
import { t } from "../../shared/i18n";
import { initialData } from "../../shared/model";
import type { Workspace } from "../../shared/types";
import { WorkbenchContext } from "../workbench/context";
import type { useWorkbenchController } from "../workbench/useWorkbenchController";
import RequestVersionDialog from "./RequestVersionDialog";

vi.mock("../../shared/api", async (original) => ({
  ...(await original<typeof import("../../shared/api")>()),
  api: vi.fn(),
}));
vi.mock("sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));
vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} });
afterEach(() => { cleanup(); vi.mocked(api).mockReset(); });

function setup(dirty = false, deleted = false) {
  const data = initialData();
  const request = data.collections[0].requests[0];
  if (deleted) data.collections[0].requests = [];
  const workspace: Workspace = { id: "workspace-a", name: "Current", revision: 2, updated_at: "2026-10-18T10:00:00Z", data };
  const state = {
    accountId: "local",
    accountRef: { current: "local" },
    saved: workspace,
    stateRef: { current: { saved: workspace, draft: workspace } },
    versionRequestId: request.id,
    dirty,
    setVersionRequestId: vi.fn(),
    setGuard: vi.fn(),
    installWorkspace: vi.fn(),
    setResponse: vi.fn(),
    setRequestId: vi.fn(),
    setSaveConflict: vi.fn(),
  };
  const base = `/api/workspaces/workspace-a/requests/${encodeURIComponent(request.id)}/versions`;
  const show = () => render(<Theme><WorkbenchContext.Provider value={state as unknown as ReturnType<typeof useWorkbenchController>}><RequestVersionDialog /></WorkbenchContext.Provider></Theme>);
  return { state, workspace, request, base, show };
}

it("loads paged request history and restores only the selected request", async () => {
  const { state, workspace, request, base, show } = setup();
  const old = { ...request, name: "Old request" };
  const restored = { ...workspace, revision: 3, data: { ...workspace.data, collections: [{ ...workspace.data.collections[0], requests: [old] }] } };
  vi.mocked(api).mockImplementation(async (path, method) => {
    if (method === "POST") return restored;
    if (path === base) return { versions: [], next_before: 5 };
    if (path === `${base}?before=5`) return { versions: [{ revision: 1, updated_at: "2026-10-17T10:00:00Z", collection_id: "c", name: old.name, method: old.method, exists: true }], next_before: null };
    if (path === `${base}/1`) return { revision: 1, updated_at: "2026-10-17T10:00:00Z", collection_id: "c", request: old };
    throw new Error(`Unexpected API path ${path}`);
  });
  show();
  fireEvent.click(await screen.findByRole("button", { name: t("加载更早版本") }));
  fireEvent.click(await screen.findByRole("button", { name: t("恢复此请求版本") }));
  fireEvent.click(screen.getByRole("button", { name: t("确认恢复") }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(`${base}/1/restore`, "POST", { expected_revision: 2 }));
  await waitFor(() => expect(state.installWorkspace).toHaveBeenCalledWith(restored));
  expect(state.setResponse).toHaveBeenCalledOnce();
  expect(state.setRequestId).toHaveBeenCalledWith(request.id);
});

it("guards unsaved changes before restoring historical request absence", async () => {
  const { state, workspace, base, show } = setup(true);
  const deleted = { ...workspace, revision: 3, data: { ...workspace.data, collections: [{ ...workspace.data.collections[0], requests: [] }] } };
  vi.mocked(api).mockImplementation(async (path, method) => {
    if (method === "POST") return deleted;
    if (path === base) return { versions: [{ revision: 1, updated_at: "2026-10-17T10:00:00Z", collection_id: null, name: null, method: null, exists: false }], next_before: null };
    if (path === `${base}/1`) return { revision: 1, updated_at: "2026-10-17T10:00:00Z", collection_id: null, request: null };
    throw new Error(`Unexpected API path ${path}`);
  });
  show();
  fireEvent.click(await screen.findByRole("button", { name: t("恢复此请求版本") }));
  fireEvent.click(screen.getByRole("button", { name: t("确认恢复") }));
  expect(vi.mocked(api).mock.calls.some((call) => call[1] === "POST")).toBe(false);
  const guard = state.setGuard.mock.calls[0][0];
  expect(guard.saveFirst).toBe(true);
  await guard.action();
  await waitFor(() => expect(state.installWorkspace).toHaveBeenCalledWith(deleted));
  expect(state.setRequestId).toHaveBeenCalled();
});

it("selects a live collection when restoring a deleted request", async () => {
  const { state, workspace, request, base, show } = setup(false, true);
  const collectionId = workspace.data.collections[0].id;
  const restored = { ...workspace, revision: 3, data: { ...workspace.data, collections: [{ ...workspace.data.collections[0], requests: [request] }] } };
  vi.mocked(api).mockImplementation(async (path, method) => {
    if (method === "POST") return restored;
    if (path === base) return { versions: [{ revision: 1, updated_at: "2026-10-17T10:00:00Z", collection_id: collectionId, name: request.name, method: request.method, exists: true }], next_before: null };
    if (path === `${base}/1`) return { revision: 1, updated_at: "2026-10-17T10:00:00Z", collection_id: collectionId, request };
    throw new Error(`Unexpected API path ${path}`);
  });
  show();
  fireEvent.click(await screen.findByRole("button", { name: t("恢复此请求版本") }));
  fireEvent.click(screen.getByRole("button", { name: t("确认恢复") }));
  await waitFor(() => expect(api).toHaveBeenCalledWith(`${base}/1/restore`, "POST", { expected_revision: 2, destination_collection_id: collectionId }));
  await waitFor(() => expect(state.installWorkspace).toHaveBeenCalledWith(restored));
});

it("ignores a restore response after an account switch", async () => {
  const { state, workspace, request, base, show } = setup();
  let complete!: (value: Workspace) => void;
  vi.mocked(api).mockImplementation(async (path, method) => {
    if (method === "POST") return new Promise<Workspace>((resolve) => { complete = resolve; });
    if (path === base) return { versions: [{ revision: 1, updated_at: "2026-10-17T10:00:00Z", collection_id: "c", name: request.name, method: request.method, exists: true }], next_before: null };
    if (path === `${base}/1`) return { revision: 1, updated_at: "2026-10-17T10:00:00Z", collection_id: "c", request: { ...request, name: "Old" } };
    throw new Error(`Unexpected API path ${path}`);
  });
  show();
  fireEvent.click(await screen.findByRole("button", { name: t("恢复此请求版本") }));
  fireEvent.click(screen.getByRole("button", { name: t("确认恢复") }));
  await waitFor(() => expect(complete).toBeTypeOf("function"));
  state.accountRef.current = "another-account";
  complete({ ...workspace, revision: 3 });
  await waitFor(() => expect(screen.queryByRole("button", { name: t("确认恢复") })).toBeNull());
  expect(state.installWorkspace).not.toHaveBeenCalled();
});
