// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, afterEach, it, expect, vi } from "vitest";
import type { PropsWithChildren } from "react";
import { api } from "../../shared/api";
import { initialData, newRequest } from "../../shared/model";
import type { Workspace } from "../../shared/types";
import { useProtocolSession } from "./useProtocolSession";
import { useWorkbenchController } from "../workbench/useWorkbenchController";
import type { useWorkspace } from "../workspaces/useWorkspace";
import type { useLocalVariables } from "../variables/useLocalVariables";
import type { ProtocolSession } from "./types";
vi.mock("../../shared/api", async (original) => ({
  ...(await original<typeof import("../../shared/api")>()),
  api: vi.fn(),
}));
vi.mock("sonner", () => ({
  toast: { error: vi.fn(), success: vi.fn(), message: vi.fn() },
}));
const request = {
  ...newRequest(),
  id: "request-a",
  url: "http://fixture/events",
  protocol: { kind: "sse" } as const,
};
const data = initialData();
data.collections[0].requests = [request];
const draft: Workspace = {
  id: "workspace-a",
  name: "fixture",
  revision: 1,
  updated_at: "now",
  data,
};
const session: ProtocolSession = {
  id: "session-a",
  workspace_id: draft.id,
  request_id: request.id,
  protocol: "sse",
  url: request.url,
  state: "open",
  reason: null,
  created_at: "now",
  updated_at: "now",
  received_bytes: 0,
  sent_bytes: 0,
  event_count: 0,
  handshake: null,
  variable_updates: [
    { scope: "project", key: "issued", value: "private-issued" },
  ],
};
let client: QueryClient;
function Wrapper({ children }: PropsWithChildren) {
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
}
beforeEach(() => {
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  vi.mocked(api).mockReset();
  for (const name of ["localStorage", "sessionStorage"]) {
    const map = new Map<string, string>();
    vi.stubGlobal(name, {
      getItem: (key: string) => map.get(key) ?? null,
      setItem: (key: string, value: string) => map.set(key, value),
      removeItem: (key: string) => map.delete(key),
      clear: () => map.clear(),
    });
  }
  window.matchMedia = vi.fn(() => ({
    matches: false,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  })) as unknown as typeof window.matchMedia;
});
afterEach(() => {
  cleanup();
  client.clear();
});
it("closes late session creation after unmount without applying its private updates", async () => {
  let resolve!: (value: ProtocolSession) => void;
  const pending = new Promise<ProtocolSession>((done) => (resolve = done));
  vi.mocked(api).mockImplementation((path) =>
    path === "/api/sessions"
      ? pending
      : Promise.resolve({ ...session, state: "closed" }),
  );
  const apply = vi.fn();
  const workspace = {
    accountId: "account-a",
    draft,
    dirty: false,
    save: vi.fn(),
  } as unknown as ReturnType<typeof useWorkspace>;
  const locals = { values: () => [], apply } as unknown as ReturnType<
    typeof useLocalVariables
  >;
  const hook = renderHook(
    () => useProtocolSession(workspace, request, locals, true),
    { wrapper: Wrapper },
  );
  let connection!: Promise<void>;
  act(() => {
    connection = hook.result.current.connect();
  });
  hook.unmount();
  await act(async () => {
    resolve(session);
    await connection;
  });
  expect(apply).not.toHaveBeenCalled();
  expect(api).toHaveBeenCalledWith("/api/sessions/session-a/close", "POST", {});
});
it("stops polling and hides the live session when authentication expires", async () => {
  sessionStorage.setItem("moleapi_token", "fixture-token");
  sessionStorage.setItem("moleapi_username", "account-a");
  vi.mocked(api).mockImplementation(async (path) => {
    if (path === "/api/auth/status")
      return {
        mode: "server",
        setup_required: false,
        registration_enabled: false,
      };
    if (path === "/api/workspaces") return [draft];
    if (path === "/api/sessions" || path === "/api/sessions/session-a")
      return session;
    if (path.startsWith("/api/sessions/session-a/events"))
      return {
        events: [],
        next_cursor: 0,
        earliest_cursor: 1,
        dropped_count: 0,
      };
    return {};
  });
  const hook = renderHook(() => useWorkbenchController(), { wrapper: Wrapper });
  await waitFor(() => expect(hook.result.current.request?.id).toBe(request.id));
  await act(async () => await hook.result.current.protocolSession.connect());
  await waitFor(() =>
    expect(
      vi.mocked(api).mock.calls.some(([path]) => path.includes("/events?")),
    ).toBe(true),
  );
  act(() => window.dispatchEvent(new Event("moleapi:unauthorized")));
  expect(hook.result.current.authenticated).toBe(false);
  expect(hook.result.current.protocolSession.session).toBeNull();
  const before = vi
    .mocked(api)
    .mock.calls.filter(([path]) => path === "/api/sessions/session-a").length;
  await act(async () => await new Promise((done) => setTimeout(done, 850)));
  expect(
    vi
      .mocked(api)
      .mock.calls.filter(([path]) => path === "/api/sessions/session-a"),
  ).toHaveLength(before);
});
