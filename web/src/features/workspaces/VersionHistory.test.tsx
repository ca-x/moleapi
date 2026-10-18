// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Theme } from "@radix-ui/themes";
import { afterEach, expect, it, vi } from "vitest";
import { api, ApiError } from "../../shared/api";
import { initialData } from "../../shared/model";
import type { Workspace } from "../../shared/types";
import { WorkbenchContext } from "../workbench/context";
import type { useWorkbenchController } from "../workbench/useWorkbenchController";
import VersionHistory from "./VersionHistory";

vi.mock("../../shared/api", async (original) => ({
  ...(await original<typeof import("../../shared/api")>()),
  api: vi.fn(),
}));
vi.mock("sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));
afterEach(() => { cleanup(); vi.mocked(api).mockReset(); });

function setup(dirty = false) {
  const current: Workspace = { id: "workspace-a", name: "Current", revision: 2, updated_at: "2026-10-18T10:00:00Z", data: initialData() };
  const prior: Workspace = { ...current, name: "Prior", revision: 1, updated_at: "2026-10-17T10:00:00Z" };
  const client = new QueryClient({ defaultOptions: { queries: { staleTime: Infinity, retry: false } } });
  client.setQueryData(["workspace-versions", "local", current.id], [prior]);
  const state = {
    accountId: "local",
    accountRef: { current: "local" },
    saved: current,
    stateRef: { current: { saved: current, draft: current } },
    modal: "versions",
    dirty,
    installWorkspace: vi.fn(),
    requestId: "removed-request",
    setRequestId: vi.fn(),
    setResponse: vi.fn(),
    setSaveConflict: vi.fn(),
    setModal: vi.fn(),
    setGuard: vi.fn(),
  };
  render(<QueryClientProvider client={client}><Theme><WorkbenchContext.Provider value={state as unknown as ReturnType<typeof useWorkbenchController>}><VersionHistory /></WorkbenchContext.Provider></Theme></QueryClientProvider>);
  return { state, current, prior };
}

it("restores a selected archived revision with the current CAS revision", async () => {
  const { state, current } = setup();
  const restored = { ...current, name: "Prior", revision: 3 };
  vi.mocked(api).mockImplementation(async (_path, method) => method === "POST" ? restored : [{ ...current, revision: 2 }]);
  fireEvent.click(screen.getByRole("button", { name: "恢复此版本" }));
  fireEvent.click(screen.getByRole("button", { name: "确认恢复" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith("/api/workspaces/workspace-a/versions/1/restore", "POST", { expected_revision: 2 }));
  await waitFor(() => expect(state.installWorkspace).toHaveBeenCalledWith(restored));
  expect(state.setResponse).toHaveBeenCalledOnce();
  expect(state.setRequestId).toHaveBeenCalledWith(restored.data.collections.flatMap((collection) => collection.requests)[0]?.id ?? "");
  expect(state.setModal).toHaveBeenCalledWith(null);
});

it("guards an unsaved draft and keeps it on a stale restore conflict", async () => {
  const { state, current } = setup(true);
  vi.mocked(api).mockRejectedValueOnce(new ApiError("conflict", 409)).mockResolvedValueOnce({ ...current, revision: 3 });
  fireEvent.click(screen.getByRole("button", { name: "恢复此版本" }));
  fireEvent.click(screen.getByRole("button", { name: "确认恢复" }));
  expect(api).not.toHaveBeenCalled();
  const guard = state.setGuard.mock.calls[0][0];
  expect(guard.saveFirst).toBe(true);
  await guard.action();
  await waitFor(() => expect(state.setSaveConflict).toHaveBeenCalledWith({ ...current, revision: 3 }));
  expect(state.installWorkspace).not.toHaveBeenCalled();
  expect(state.stateRef.current.draft).toBe(current);
});

it("does not install a restore response after switching accounts", async () => {
  const { state, current } = setup();
  let complete!: (value: Workspace) => void;
  vi.mocked(api).mockReturnValue(new Promise<Workspace>((resolve) => { complete = resolve; }));
  fireEvent.click(screen.getByRole("button", { name: "恢复此版本" }));
  fireEvent.click(screen.getByRole("button", { name: "确认恢复" }));
  await waitFor(() => expect(api).toHaveBeenCalledTimes(1));
  state.accountRef.current = "another-account";
  complete({ ...current, revision: 3 });
  await waitFor(() => expect(screen.queryByRole("button", { name: "确认恢复" })).toBeNull());
  expect(state.installWorkspace).not.toHaveBeenCalled();
});
