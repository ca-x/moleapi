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
import { useWorkspace } from "../workspaces/useWorkspace";
import type { useLocalVariables } from "../variables/useLocalVariables";
import type { EventBatch, ProtocolSession } from "./types";
import { dataConfig } from "../data/model";
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
it("sends Data cancellation while a query preparation request is pending", async () => {
  const dataRequest = {...request, protocol:dataConfig()};
  const dataSession = {...session, protocol:"data" as const};
  let finishQuery!: (value:unknown)=>void;
  vi.mocked(api).mockImplementation((path,method,body) => {
    if(path==="/api/sessions"&&method==="POST") return Promise.resolve(dataSession);
    if(path.endsWith("/events?after=0")) return Promise.resolve({events:[],next_cursor:0,earliest_cursor:1,dropped_count:0});
    if(path==="/api/sessions/session-a/send"&&method==="POST"&&body&&typeof body==="object"&&"kind" in body&&body.kind==="data_query") return new Promise(done=>{finishQuery=done;});
    if(path==="/api/sessions/session-a/send"&&method==="POST") return Promise.resolve({ok:true});
    return Promise.resolve(dataSession);
  });
  const workspace = {accountId:"account-a",draft,dirty:false,save:vi.fn()} as unknown as ReturnType<typeof useWorkspace>;
  const locals = {values:()=>[],apply:vi.fn()} as unknown as ReturnType<typeof useLocalVariables>;
  const hook = renderHook(()=>useProtocolSession(workspace,dataRequest,locals,true),{wrapper:Wrapper});
  await act(async()=>await hook.result.current.connect());
  let pending!:Promise<boolean>;
  act(()=>{pending=hook.result.current.send({kind:"data_query",query_id:"query",sql:"SELECT 1",read_only:true});});
  await waitFor(()=>expect(hook.result.current.sending).toBe(true));
  await act(async()=>expect(await hook.result.current.send({kind:"data_cancel",query_id:"query"})).toBe(true));
  expect(api).toHaveBeenCalledWith("/api/sessions/session-a/send","POST",{kind:"data_cancel",query_id:"query"});
  await act(async()=>{finishQuery({ok:true});await pending;});
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

it.each([false, true])(
  "Stop fences pending gRPC creation with retained terminal session=%s",
  async (retained) => {
    const grpcRequest = {
      ...request,
      protocol: {
        kind: "grpc" as const,
        service: "Fixture",
        method: "Unary",
        message_source: "{}",
      },
    };
    const grpcSession = {
      ...session,
      protocol: "grpc" as const,
      state: "closed" as const,
    };
    let resolve!: (value: ProtocolSession) => void;
    let creations = 0;
    vi.mocked(api).mockImplementation(async (path) => {
      if (path === "/api/sessions") {
        creations++;
        if (retained && creations === 1) return grpcSession;
        return new Promise<ProtocolSession>((done) => {
          resolve = done;
        });
      }
      if (path.includes("/events"))
        return {
          events: [],
          next_cursor: 0,
          earliest_cursor: 1,
          dropped_count: 0,
        };
      return grpcSession;
    });
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
      () => useProtocolSession(workspace, grpcRequest, locals, true),
      { wrapper: Wrapper },
    );
    if (retained)
      await act(async () => {
        await hook.result.current.connect();
      });
    apply.mockClear();
    let pending!: Promise<void>;
    act(() => {
      pending = hook.result.current.connect();
    });
    expect(hook.result.current.busy).toBe(true);
    await act(async () => {
      await hook.result.current.close();
    });
    expect(hook.result.current.busy).toBe(false);
    await act(async () => {
      resolve({ ...grpcSession, id: "late-grpc", state: "open" });
      await pending;
    });
    expect(hook.result.current.session).toBeNull();
    expect(apply).not.toHaveBeenCalled();
    expect(api).toHaveBeenCalledWith(
      "/api/sessions/late-grpc/close",
      "POST",
      {},
    );
  },
);

it.each([["closed", "open"], ["closed", "connecting"], ["error", "open"], ["error", "connecting"]] as const)(
  "does not reopen a %s session when an earlier %s poll finishes",
  async (state, staleState) => {
    let finishEvents!: (value: unknown) => void;
    const finalEvents = new Promise((resolve) => { finishEvents = resolve; });
    const terminal = { ...session, state, reason: "Finished" };
    vi.mocked(api).mockImplementation(async (path, method) => {
      if (path.endsWith("/close") && method === "POST") return terminal;
      if (path.includes("/events?")) return finalEvents;
      return path === "/api/sessions" ? session : { ...session, state: staleState };
    });
    const workspace = { accountId: "account-a", draft, dirty: false, save: vi.fn() } as unknown as ReturnType<typeof useWorkspace>;
    const locals = { values: () => [], apply: vi.fn() } as unknown as ReturnType<typeof useLocalVariables>;
    const hook = renderHook(() => useProtocolSession(workspace, request, locals, true), { wrapper: Wrapper });
    await act(async () => { await hook.result.current.connect(); });
    await waitFor(() => expect(finishEvents).toBeTypeOf("function"));
    await waitFor(() => expect(vi.mocked(api).mock.calls.some(([path]) => path.includes("/events?"))).toBe(true));
    await act(async () => { await hook.result.current.close(); });
    expect(hook.result.current.session?.state).toBe(state);
    await act(async () => {
      finishEvents({ events: [{ cursor: 7, received_at: "now", direction: "incoming", message: { kind: "text", text: "final event" } }], next_cursor: 7, earliest_cursor: 1, dropped_count: 0 });
    });
    await waitFor(() => expect(hook.result.current.events).toHaveLength(1));
    expect(hook.result.current.session?.state).toBe(state);
    expect(hook.result.current.session?.reason).toBe("Finished");
    const polls = vi.mocked(api).mock.calls.filter(([path, method]) => path === "/api/sessions/session-a" && !method).length;
    await act(async () => { await new Promise((resolve) => setTimeout(resolve, 850)); });
    expect(vi.mocked(api).mock.calls.filter(([path, method]) => path === "/api/sessions/session-a" && !method)).toHaveLength(polls);
  },
);

it("closes a live session when the same workspace is explicitly replaced", async () => {
  vi.mocked(api).mockImplementation(async (path) => path.includes("/events?")
    ? { events: [], next_cursor: 0, earliest_cursor: 1, dropped_count: 0 }
    : session);
  const workspace = { accountId: "account-a", workspaceGeneration: 1, draft, dirty: false, save: vi.fn() } as unknown as ReturnType<typeof useWorkspace>;
  const locals = { values: () => [], apply: vi.fn() } as unknown as ReturnType<typeof useLocalVariables>;
  const hook = renderHook(({ current }) => useProtocolSession(current, request, locals, true), { initialProps: { current: workspace }, wrapper: Wrapper });
  await act(async () => { await hook.result.current.connect(); });
  expect(hook.result.current.session?.id).toBe(session.id);
  hook.rerender({ current: { ...workspace, workspaceGeneration: 2, draft: { ...draft, revision: 2 } } });
  expect(hook.result.current.session).toBeNull();
  await waitFor(() => expect(api).toHaveBeenCalledWith("/api/sessions/session-a/close", "POST", {}));
});

it("closes each captured session on request change and unmount", async () => {
  const secondRequest = { ...request, id: "request-b" };
  const secondSession = { ...session, id: "session-b", request_id: secondRequest.id };
  vi.mocked(api).mockImplementation(async (path, method, body) => {
    if (path === "/api/sessions" && method === "POST") return (body as { request: { id: string } }).request.id === request.id ? session : secondSession;
    if (path.includes("/events?")) return { events: [], next_cursor: 0, earliest_cursor: 1, dropped_count: 0 };
    return path.includes("session-b") ? secondSession : session;
  });
  const workspace = { accountId: "account-a", draft, dirty: false, save: vi.fn() } as unknown as ReturnType<typeof useWorkspace>;
  const locals = { values: () => [], apply: vi.fn() } as unknown as ReturnType<typeof useLocalVariables>;
  const hook = renderHook(({ selected }) => useProtocolSession(workspace, selected, locals, true), { initialProps: { selected: request }, wrapper: Wrapper });
  await act(async () => { await hook.result.current.connect(); });
  hook.rerender({ selected: secondRequest });
  await act(async () => { await hook.result.current.connect(); });
  expect(hook.result.current.session?.id).toBe(secondSession.id);
  expect(vi.mocked(api).mock.calls.filter(([path]) => path === "/api/sessions/session-a/close")).toHaveLength(1);
  expect(vi.mocked(api).mock.calls.filter(([path]) => path === "/api/sessions/session-b/close")).toHaveLength(0);
  hook.unmount();
  expect(vi.mocked(api).mock.calls.filter(([path]) => path === "/api/sessions/session-b/close")).toHaveLength(1);
});

it("preserves the live connection when a normal save advances the workspace revision", async () => {
  vi.mocked(api).mockImplementation(async (path) => path.includes("/events?")
    ? { events: [], next_cursor: 0, earliest_cursor: 1, dropped_count: 0 }
    : session);
  const workspace = { accountId: "account-a", workspaceGeneration: 1, draft, dirty: false, save: vi.fn() } as unknown as ReturnType<typeof useWorkspace>;
  const locals = { values: () => [], apply: vi.fn() } as unknown as ReturnType<typeof useLocalVariables>;
  const hook = renderHook(({ current }) => useProtocolSession(current, request, locals, true), { initialProps: { current: workspace }, wrapper: Wrapper });
  await act(async () => { await hook.result.current.connect(); });
  hook.rerender({ current: { ...workspace, draft: { ...draft, revision: 2 } } });
  expect(hook.result.current.session?.id).toBe(session.id);
  expect(vi.mocked(api).mock.calls.filter(([path]) => path.endsWith("/close"))).toHaveLength(0);
});

it("closes a late session creation after replacing the same workspace without applying private updates", async () => {
  let finish!: (value: ProtocolSession) => void;
  vi.mocked(api).mockImplementation((path) => path === "/api/sessions"
    ? new Promise<ProtocolSession>((resolve) => { finish = resolve; })
    : Promise.resolve({ ...session, state: "closed" }));
  const workspaceGenerationRef = { current: 1 };
  const workspace = { accountId: "account-a", workspaceGeneration: 1, workspaceGenerationRef, draft, dirty: false, save: vi.fn() } as unknown as ReturnType<typeof useWorkspace>;
  const locals = { values: () => [], apply: vi.fn() } as unknown as ReturnType<typeof useLocalVariables>;
  const hook = renderHook(({ current }) => useProtocolSession(current, request, locals, true), { initialProps: { current: workspace }, wrapper: Wrapper });
  let connecting!: Promise<void>;
  act(() => { connecting = hook.result.current.connect(); });
  await waitFor(() => expect(finish).toBeTypeOf("function"));
  await act(async () => {
    workspaceGenerationRef.current = 2;
    hook.rerender({ current: { ...workspace, workspaceGeneration: 2, draft: { ...draft, revision: 3 } } });
    finish(session);
    await connecting;
  });
  expect(hook.result.current.session).toBeNull();
  expect(locals.apply).not.toHaveBeenCalled();
  expect(api).toHaveBeenCalledWith("/api/sessions/session-a/close", "POST", {});
});


it("fetches final events after Stop when the initial pending poll captured an empty snapshot", async () => {
  const empty: EventBatch = { events: [], next_cursor: 0, earliest_cursor: 1, dropped_count: 0 };
  const finalBatch: EventBatch = {
    ...empty,
    next_cursor: 1,
    events: [{ cursor: 1, received_at: "now", direction: "system", message: { kind: "state", state: "closed", reason: "Finished" } }],
  };
  const terminal = { ...session, state: "closed" as const, reason: "Finished", event_count: 1 };
  let finishFirst!: (value: EventBatch) => void;
  const firstBatch = new Promise<EventBatch>((resolve) => { finishFirst = resolve; });
  let closed = false;
  let eventReads = 0;
  vi.mocked(api).mockImplementation(async (path, method) => {
    if (path === "/api/sessions") return session;
    if (path.endsWith("/close") && method === "POST") {
      closed = true;
      return terminal;
    }
    if (path.includes("/events?")) {
      eventReads++;
      return eventReads === 1 ? firstBatch : finalBatch;
    }
    return closed ? terminal : session;
  });
  const workspace = { accountId: "account-a", draft, dirty: false, save: vi.fn() } as unknown as ReturnType<typeof useWorkspace>;
  const locals = { values: () => [], apply: vi.fn() } as unknown as ReturnType<typeof useLocalVariables>;
  const hook = renderHook(() => useProtocolSession(workspace, request, locals, true), { wrapper: Wrapper });
  await act(async () => { await hook.result.current.connect(); });
  await waitFor(() => expect(eventReads).toBe(1));
  await act(async () => { await hook.result.current.close(); });
  expect(hook.result.current.session?.state).toBe("closed");
  await act(async () => {
    finishFirst(empty);
    await firstBatch;
  });
  await waitFor(() => expect(hook.result.current.events).toEqual(finalBatch.events));
  expect(eventReads).toBe(2);
  expect(hook.result.current.session?.state).toBe("closed");
});

it("rejects a retained send before any POST when the same workspace is replaced before rerender", async () => {
  const socketRequest = { ...request, protocol: { kind: "websocket" } as const };
  const original = { ...draft, data: { ...data, collections: [{ ...data.collections[0], requests: [socketRequest] }] } };
  const opened = { ...session, protocol: "websocket" as const };
  client.setQueryData(["workspaces", "local"], [original]);
  vi.mocked(api).mockImplementation(async (path) => {
    if (path === "/api/workspaces") return [original];
    if (path.includes("/events?")) return { events: [], next_cursor: 0, earliest_cursor: 1, dropped_count: 0 };
    return opened;
  });
  const locals = { values: () => [], apply: vi.fn() } as unknown as ReturnType<typeof useLocalVariables>;
  const hook = renderHook(() => {
    const workspace = useWorkspace(true);
    const selected = workspace.draft?.data.collections[0].requests[0];
    return { workspace, channel: useProtocolSession(workspace, selected, locals, true) };
  }, { wrapper: Wrapper });
  await waitFor(() => expect(hook.result.current.workspace.draft).toBeDefined());
  await act(async () => { await hook.result.current.channel.connect(); });
  expect(hook.result.current.channel.session?.state).toBe("open");
  const retainedSend = hook.result.current.channel.send;
  let accepted: boolean | undefined;
  await act(async () => {
    hook.result.current.workspace.installWorkspace({ ...original, revision: 3 });
    accepted = await retainedSend({ kind: "text", text: "stale workspace command" });
  });
  expect(accepted).toBe(false);
  expect(vi.mocked(api).mock.calls.filter(([path]) => path === "/api/sessions/session-a/send")).toHaveLength(0);
  expect(hook.result.current.channel.session).toBeNull();
});
