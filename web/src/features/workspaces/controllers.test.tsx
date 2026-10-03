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
import { useLocalVariables } from "../variables/useLocalVariables";
import { useRunner } from "../testing/useRunner";
import { useSync } from "../sync/useSync";
const nativeMode = vi.hoisted(() => ({ enabled: false }));
vi.mock("../../shared/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../shared/api")>()),
  api: vi.fn(),
  get native() {
    return nativeMode.enabled;
  },
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
  nativeMode.enabled = false;
  mockedApi.mockReset();
  const storage = new Map<string, string>();
  vi.stubGlobal("localStorage", {
    getItem: (key: string) => storage.get(key) ?? null,
    setItem: (key: string, value: string) => storage.set(key, value),
    removeItem: (key: string) => storage.delete(key),
    clear: () => storage.clear(),
  });
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

describe("script local variable execution", () => {
  it("applies collection-run extracted variables locally without mutating the shared draft", async () => {
    const original = makeWorkspace();
    mockedApi.mockResolvedValue({
      results: [
        {
          request_id: original.data.collections[0].requests[0].id,
          request_name: "request",
          response: {
            ...response,
            variable_updates: [
              { scope: "environment", key: "issued", value: "private-issued" },
            ],
          },
        },
      ],
      passed: 1,
      failed: 0,
      elapsed_ms: 1,
    } satisfies RunResult);
    const { result } = renderHook(
      () => {
        const workspace = useWorkspace(true);
        const locals = useLocalVariables(workspace);
        return { workspace, locals, runner: useRunner(workspace, locals) };
      },
      { wrapper: harness(original) },
    );
    await waitFor(() =>
      expect(result.current.workspace.draft?.id).toBe(original.id),
    );
    await act(async () => {
      await result.current.runner.run();
    });
    expect(result.current.locals.read("environment", "local", "issued")).toBe(
      "private-issued",
    );
    expect(result.current.workspace.draft?.data).toEqual(original.data);
    expect(result.current.workspace.dirty).toBe(false);
  });
  it("discards delayed variable changes after switching workspaces", async () => {
    const original = makeWorkspace();
    const pending = deferred<ApiResponse>();
    mockedApi.mockReturnValue(pending.promise);
    const { result } = renderHook(
      () => {
        const workspace = useWorkspace(true);
        const locals = useLocalVariables(workspace);
        return {
          workspace,
          locals,
          requests: useRequests(workspace, vi.fn(), vi.fn(), locals),
        };
      },
      { wrapper: harness(original) },
    );
    await waitFor(() => expect(result.current.requests.request).toBeDefined());
    let sending!: Promise<void>;
    act(() => {
      sending = result.current.requests.send();
    });
    act(() =>
      result.current.workspace.installWorkspace({
        ...original,
        id: "workspace-b",
      }),
    );
    await act(async () => {
      pending.resolve({
        ...response,
        variable_updates: [
          {
            scope: "environment",
            key: "issued",
            value: "old-workspace-secret",
          },
        ],
      });
      await sending;
    });
    expect(
      result.current.locals.read("environment", "local", "issued"),
    ).toBeUndefined();
    expect(
      localStorage.getItem("moleapi:local-values:local:workspace-a"),
    ).toBeNull();
    expect(
      localStorage.getItem("moleapi:local-values:local:workspace-b"),
    ).toBeNull();
  });
});

it("retains the rendered draft and old local values when browser storage is full", async () => {
  const original = makeWorkspace();
  const { result } = renderHook(
    () => {
      const workspace = useWorkspace(true);
      return { workspace, locals: useLocalVariables(workspace) };
    },
    { wrapper: harness(original) },
  );
  await waitFor(() =>
    expect(result.current.workspace.draft?.id).toBe(original.id),
  );
  act(() =>
    result.current.locals.write(
      "environment",
      "local",
      "credential",
      "previous-private",
    ),
  );
  vi.spyOn(localStorage, "setItem").mockImplementation(() => {
    throw new DOMException("full", "QuotaExceededError");
  });
  expect(() =>
    act(() =>
      result.current.locals.write(
        "environment",
        "local",
        "credential",
        "new-private",
      ),
    ),
  ).not.toThrow();
  expect(result.current.locals.read("environment", "local", "credential")).toBe(
    "previous-private",
  );
  expect(result.current.workspace.draft).toEqual(original);
});

it("keeps native blank-name local overrides attached to their row identity", async () => {
  nativeMode.enabled = true;
  const original = makeWorkspace();
  original.data.global_variables = [
    { id: "a", key: "", value: "", enabled: true, local_value: "private-a" },
    { id: "b", key: "", value: "", enabled: true, local_value: "private-b" },
  ];
  const { result } = renderHook(
    () => {
      const workspace = useWorkspace(true);
      return { workspace, locals: useLocalVariables(workspace) };
    },
    { wrapper: harness(original) },
  );
  await waitFor(() =>
    expect(result.current.workspace.draft?.id).toBe(original.id),
  );
  expect(result.current.locals.read("project", "", "", "b")).toBe("private-b");
  act(() => result.current.locals.write("project", "", "", "edited-a", "a"));
  expect(
    result.current.workspace.draft?.data.global_variables?.map(
      (row) => row.local_value,
    ),
  ).toEqual(["edited-a", "private-b"]);
});

it("keeps extracted local values in the environment used by the original request", async () => {
  const original = makeWorkspace();
  original.data.environments.push({
    id: "production",
    name: "生产",
    variables: [],
  });
  const pending = deferred<ApiResponse>();
  mockedApi.mockReturnValue(pending.promise);
  const { result } = renderHook(
    () => {
      const workspace = useWorkspace(true);
      const locals = useLocalVariables(workspace);
      return {
        workspace,
        locals,
        requests: useRequests(workspace, vi.fn(), vi.fn(), locals),
      };
    },
    { wrapper: harness(original) },
  );
  await waitFor(() => expect(result.current.requests.request).toBeDefined());
  let sending!: Promise<void>;
  act(() => {
    sending = result.current.requests.send();
  });
  act(() =>
    result.current.workspace.updateData((data) => ({
      ...data,
      active_environment_id: "production",
    })),
  );
  await act(async () => {
    pending.resolve({
      ...response,
      variable_updates: [
        {
          scope: "environment",
          key: "api_token",
          value: "development-response-token",
        },
      ],
    });
    await sending;
  });
  expect(result.current.locals.read("environment", "local", "api_token")).toBe(
    "development-response-token",
  );
  expect(
    result.current.locals.read("environment", "production", "api_token"),
  ).toBeUndefined();
  expect(result.current.workspace.draft?.data.active_environment_id).toBe(
    "production",
  );
});
