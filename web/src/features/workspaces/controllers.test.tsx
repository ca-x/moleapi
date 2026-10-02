// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { PropsWithChildren } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api, ApiError } from "../../shared/api";
import { initialData } from "../../shared/model";
import type { ApiResponse, RunResult, Workspace } from "../../shared/types";
import { useWorkspace } from "./useWorkspace";
import { useRequests } from "../requests/useRequests";
import { useRunner } from "../testing/useRunner";
import { useSync } from "../sync/useSync";
vi.mock("../../shared/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../shared/api")>()),
  api: vi.fn(),
}));
vi.mock("sonner", () => ({
  toast: { error: vi.fn(), success: vi.fn(), message: vi.fn() },
}));
const mockedApi = vi.mocked(api);
const makeWorkspace = (): Workspace => ({
  id: "workspace-a",
  name: "Original",
  revision: 1,
  updated_at: "2026-10-02",
  data: initialData(),
});
const response: ApiResponse = {
  status: 200,
  status_text: "OK",
  headers: [],
  body: '{"done":true}',
  elapsed_ms: 12,
  size_bytes: 13,
  truncated: false,
  url: "https://example.com",
  tests: [],
};
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
function harness(workspace: Workspace) {
  const client = new QueryClient({
    defaultOptions: {
      queries: { retry: false, staleTime: Infinity },
      mutations: { retry: false },
    },
  });
  client.setQueryData(["workspaces", "local"], [workspace]);
  client.setQueryData(["sync-status"], { connected: true });
  return function Wrapper({ children }: PropsWithChildren) {
    return (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
  };
}
beforeEach(() => {
  mockedApi.mockReset();
});
afterEach(cleanup);
describe("frontend async controllers", () => {
  it("saves once for concurrent calls and keeps changes typed during the network wait", async () => {
    const original = makeWorkspace();
    const pending = deferred<Workspace>();
    mockedApi.mockReturnValue(pending.promise);
    const { result } = renderHook(() => useWorkspace(true), {
      wrapper: harness(original),
    });
    await waitFor(() => expect(result.current.draft?.id).toBe(original.id));
    act(() =>
      result.current.setDraft(
        (current) => current && { ...current, name: "Sent name" },
      ),
    );
    let first!: Promise<Workspace | null>;
    let second!: Promise<Workspace | null>;
    act(() => {
      first = result.current.save();
      second = result.current.save();
    });
    expect(first).toBe(second);
    expect(mockedApi).toHaveBeenCalledTimes(1);
    act(() =>
      result.current.setDraft(
        (current) => current && { ...current, name: "New typing" },
      ),
    );
    await act(async () => {
      pending.resolve({ ...original, name: "Sent name", revision: 2 });
      await first;
    });
    expect(result.current.draft?.name).toBe("New typing");
    expect(result.current.draft?.revision).toBe(2);
    expect(result.current.saved?.name).toBe("Sent name");
    expect(result.current.dirty).toBe(true);
  });
  it("retains edits on a failed save and offers a remote revision for an explicit retry", async () => {
    const original = makeWorkspace();
    const remote = { ...original, name: "Remote", revision: 3 };
    mockedApi.mockImplementation(async (_path, method) => {
      if (method === "PUT") throw new ApiError("Conflict", 409);
      return remote;
    });
    const { result } = renderHook(() => useWorkspace(true), {
      wrapper: harness(original),
    });
    await waitFor(() => expect(result.current.draft).not.toBe(null));
    act(() =>
      result.current.setDraft(
        (current) => current && { ...current, name: "My edit" },
      ),
    );
    await act(async () => {
      expect(await result.current.save()).toBe(null);
    });
    expect(result.current.draft?.name).toBe("My edit");
    expect(result.current.saveConflict?.revision).toBe(3);
    act(() => result.current.keepConflictEdits());
    expect(result.current.draft?.name).toBe("My edit");
    expect(result.current.draft?.revision).toBe(3);
    expect(result.current.dirty).toBe(true);
  });
  it("runs the initially displayed collection on the first click", async () => {
    const original = makeWorkspace();
    const value: RunResult = {
      results: [],
      passed: 1,
      failed: 0,
      elapsed_ms: 5,
    };
    mockedApi.mockResolvedValue(value);
    const { result } = renderHook(
      () => {
        const workspace = useWorkspace(true);
        return useRunner(workspace);
      },
      { wrapper: harness(original) },
    );
    await waitFor(() =>
      expect(result.current.runCollection).toBe(
        original.data.collections[0].id,
      ),
    );
    await act(async () => {
      await result.current.run();
    });
    expect(mockedApi).toHaveBeenCalledWith(
      `/api/workspaces/${original.id}/run`,
      "POST",
      {
        collection_id: original.data.collections[0].id,
        environment_id: "local",
      },
    );
    expect(result.current.runResult).toEqual(value);
  });
  it("never displays a delayed response under a different request or workspace", async () => {
    const original = makeWorkspace();
    const pending = deferred<ApiResponse>();
    mockedApi.mockReturnValue(pending.promise);
    const { result } = renderHook(
      () => {
        const workspace = useWorkspace(true);
        return {
          workspace,
          requests: useRequests(workspace, vi.fn(), vi.fn()),
        };
      },
      { wrapper: harness(original) },
    );
    await waitFor(() =>
      expect(result.current.requests.request).not.toBe(undefined),
    );
    let sending!: Promise<void>;
    act(() => {
      sending = result.current.requests.send();
    });
    act(() =>
      result.current.requests.setRequestId(
        original.data.collections[0].requests[1].id,
      ),
    );
    await act(async () => {
      pending.resolve(response);
      await sending;
    });
    expect(result.current.requests.response).toBe(null);
    act(() =>
      result.current.requests.setRequestId(
        original.data.collections[0].requests[0].id,
      ),
    );
    expect(result.current.requests.response).toEqual(response);
    act(() =>
      result.current.workspace.installWorkspace({
        ...original,
        id: "workspace-b",
      }),
    );
    expect(result.current.requests.response).toBe(null);
  });
});

describe("sync and save network boundaries", () => {
  it("preserves changes made during synchronization and offers the returned database version", async () => {
    const original = makeWorkspace();
    const pending = deferred<unknown>();
    mockedApi.mockReturnValue(pending.promise);
    const { result } = renderHook(
      () => {
        const workspace = useWorkspace(true);
        return {
          workspace,
          synchronization: useSync(true, workspace, vi.fn()),
        };
      },
      { wrapper: harness(original) },
    );
    await waitFor(() => expect(result.current.workspace.draft).not.toBe(null));
    let synchronization!: Promise<void>;
    act(() => {
      synchronization = result.current.synchronization.synchronize();
    });
    act(() =>
      result.current.workspace.setDraft(
        (current) => current && { ...current, name: "Typed during sync" },
      ),
    );
    await act(async () => {
      pending.resolve({
        status: "synced",
        workspace: { ...original, revision: 2 },
        message: "Done",
      });
      await synchronization;
    });
    expect(result.current.workspace.draft?.name).toBe("Typed during sync");
    expect(result.current.workspace.dirty).toBe(true);
    expect(result.current.workspace.saveConflict?.revision).toBe(2);
  });
  it("keeps the complete editable draft after a network save failure", async () => {
    const original = makeWorkspace();
    mockedApi.mockRejectedValue(new Error("Offline"));
    const { result } = renderHook(() => useWorkspace(true), {
      wrapper: harness(original),
    });
    await waitFor(() => expect(result.current.draft).not.toBe(null));
    act(() =>
      result.current.updateData((data) => ({ ...data, collections: [] })),
    );
    const expected = structuredClone(result.current.draft);
    await act(async () => {
      await result.current.save();
    });
    expect(result.current.draft).toEqual(expected);
    expect(result.current.saved).toEqual(original);
    expect(result.current.dirty).toBe(true);
  });
  it("does not open an old conflict over a different workspace selected during a save", async () => {
    const original = makeWorkspace();
    const conflict = deferred<Workspace>();
    mockedApi.mockImplementation(async (_path, method) => {
      if (method === "PUT") throw new ApiError("Conflict", 409);
      return conflict.promise;
    });
    const { result } = renderHook(() => useWorkspace(true), {
      wrapper: harness(original),
    });
    await waitFor(() => expect(result.current.draft).not.toBe(null));
    let saving!: Promise<Workspace | null>;
    act(() => {
      saving = result.current.save();
    });
    await waitFor(() => expect(mockedApi).toHaveBeenCalledTimes(2));
    act(() =>
      result.current.installWorkspace({ ...original, id: "workspace-b" }),
    );
    await act(async () => {
      conflict.resolve({ ...original, revision: 4 });
      await saving;
    });
    expect(result.current.draft?.id).toBe("workspace-b");
    expect(result.current.saveConflict).toBe(null);
  });
});

describe("account boundaries", () => {
  it("keeps expired-session drafts private and restores them when returning to the same account", async () => {
    const original = makeWorkspace();
    mockedApi.mockResolvedValue([]);
    const { result, rerender } = renderHook(
      ({ account }) => useWorkspace(true, account),
      { initialProps: { account: "local" }, wrapper: harness(original) },
    );
    await waitFor(() => expect(result.current.draft?.id).toBe(original.id));
    act(() =>
      result.current.setDraft(
        (current) => current && { ...current, name: "Private unsaved edit" },
      ),
    );
    rerender({ account: "another-user" });
    await waitFor(() => expect(result.current.draft).toBe(null));
    rerender({ account: "local" });
    await waitFor(() =>
      expect(result.current.draft?.name).toBe("Private unsaved edit"),
    );
    expect(result.current.dirty).toBe(true);
  });
});
