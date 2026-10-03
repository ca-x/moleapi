import { describe, it, expect, vi, beforeEach } from "vitest";
import { api } from "../../shared/api";
import { newRequest } from "../../shared/model";
import type { ApiResponse } from "../../shared/types";
import { createRustGraphQLFetcher } from "./fetcher";
import type { GraphQLContext } from "./fetcher";
vi.mock("../../shared/api", () => ({ api: vi.fn() }));
const response = (body: string, status = 200): ApiResponse => ({
  status,
  status_text: "",
  headers: [],
  body,
  elapsed_ms: 1,
  size_bytes: body.length,
  truncated: false,
  url: "https://fixture/graphql",
  tests: [],
});
const context = (): GraphQLContext => ({
  workspace_id: "workspace",
  environment_id: "development",
  request: {
    ...newRequest(),
    id: "request",
    url: "https://fixture/graphql",
    protocol: {
      kind: "graphql",
      document: "query { value }",
      variables: {},
      connection_params: {},
    },
  },
  locals: [{ scope: "environment", key: "token", value: "local-token" }],
  current: () => true,
  apply: vi.fn(),
  response: vi.fn(),
  sessions: new Set(),
});
beforeEach(() => {
  vi.mocked(api).mockReset();
});
describe("Rust-backed GraphQL fetcher", () => {
  it("preserves target401 as a GraphQL result without changing MoleAPI authentication", async () => {
    const state = context();
    const result = response('{"errors":[{"message":"Unauthorized"}]}', 401);
    vi.mocked(api).mockResolvedValue(result);
    const fetcher = createRustGraphQLFetcher(async () => state);
    expect(
      await fetcher({ query: "query { value }", variables: { id: 1 } }),
    ).toEqual({ errors: [{ message: "Unauthorized" }] });
    expect(state.response).toHaveBeenCalledWith(result);
    expect(api).toHaveBeenCalledWith(
      "/api/execute",
      "POST",
      expect.objectContaining({
        environment_id: "development",
        locals: state.locals,
        request: expect.objectContaining({
          protocol: expect.objectContaining({ variables_source: '{"id":1}' }),
        }),
      }),
    );
  });
  it("does not accept arbitrary HTTP JSON as a GraphQL response", async () => {
    vi.mocked(api).mockResolvedValue(response('{"success":true}'));
    await expect(
      createRustGraphQLFetcher(async () => context())({ query: "{value}" }),
    ).rejects.toThrow("data/errors");
  });
  it("discards delayed finite result and variable updates after scope changes", async () => {
    const state = context();
    let active = true;
    state.current = () => active;
    let resolve!: (value: ApiResponse) => void;
    vi.mocked(api).mockReturnValue(new Promise((done) => (resolve = done)));
    const result = createRustGraphQLFetcher(async () => state)({
      query: "{value}",
    });
    await Promise.resolve();
    await Promise.resolve();
    active = false;
    resolve({
      ...response('{"data":{"value":1}}'),
      variable_updates: [
        { scope: "environment", key: "token", value: "late-private" },
      ],
    });
    await expect(result).rejects.toThrow("取消");
    expect(state.apply).not.toHaveBeenCalled();
    expect(state.response).not.toHaveBeenCalled();
  });
  it("closes a subscription after GraphiQL consumes completion and preserves next payloads", async () => {
    const state = context();
    const session = { id: "subscription", state: "closed", reason: null };
    vi.mocked(api).mockImplementation(async (path) => {
      if (path === "/api/sessions") return session;
      if (path.endsWith("/events?after=0"))
        return {
          events: [
            {
              cursor: 1,
              message: {
                kind: "graphql_next",
                operation_id: "operation",
                payload: { data: { value: 2 } },
              },
            },
            {
              cursor: 2,
              message: {
                kind: "graphql_complete",
                operation_id: "operation",
                payload: null,
              },
            },
          ],
          next_cursor: 2,
          earliest_cursor: 1,
          dropped_count: 0,
        };
      return session;
    });
    const result = await createRustGraphQLFetcher(async () => state)({
      query: "subscription {value}",
    });
    const values = [];
    for await (const value of result as AsyncIterable<unknown>)
      values.push(value);
    expect(values).toEqual([{ data: { value: 2 } }]);
    expect(state.sessions.size).toBe(0);
    expect(api).toHaveBeenCalledWith(
      "/api/sessions/subscription/close",
      "POST",
      {},
    );
  });
});

it("Stop closes a quiet subscription while its next() is waiting for a poll", async () => {
  const state = context();
  let polling!: () => void;
  const pollStarted = new Promise<void>((resolve) => { polling = resolve; });
  let finishPoll!: (value: unknown) => void;
  vi.mocked(api).mockImplementation(async (path) => {
    if (path === "/api/sessions") return { id: "quiet", state: "open" };
    if (path === "/api/sessions/quiet") {
      polling();
      return new Promise((resolve) => { finishPoll = resolve; });
    }
    return {};
  });
  const stream = await createRustGraphQLFetcher(async () => state)({ query: "subscription {value}" }) as AsyncIterableIterator<unknown>;
  const next = stream.next();
  await pollStarted;
  expect(await stream.return!()).toEqual({ done: true, value: undefined });
  expect(api).toHaveBeenCalledWith("/api/sessions/quiet/close", "POST", {});
  finishPoll({ id: "quiet", state: "closed" });
  expect((await next).done).toBe(true);
  expect(state.sessions.size).toBe(0);
});
it("Stop before session creation closes the late session without applying its variables", async () => {
  const state = context();
  let created!: (value: unknown) => void;
  vi.mocked(api).mockImplementation(async (path) => {
    if (path === "/api/sessions") return new Promise((resolve) => { created = resolve; });
    return {};
  });
  const stream = await createRustGraphQLFetcher(async () => state)({ query: "subscription {value}" }) as AsyncIterableIterator<unknown>;
  const next = stream.next();
  await stream.return!();
  created({ id: "late", state: "open", variable_updates: [{scope: "environment", key: "token", value: "late"}] });
  expect((await next).done).toBe(true);
  expect(api).toHaveBeenCalledWith("/api/sessions/late/close", "POST", {});
  expect(state.sessions.size).toBe(0);
  expect(state.apply).not.toHaveBeenCalled();
});

it("executes the current document when immediate Send receives stale GraphiQL operation facts", async () => {
  vi.mocked(api).mockResolvedValue(response('{"data":{"add":2}}'));
  const result = await createRustGraphQLFetcher(async () => context())({
    query: "mutation Increment { add(value:2) }", operationName: "Hello",
  });
  expect(result).toEqual({data: {add: 2}});
  expect(api).toHaveBeenCalledWith("/api/execute", "POST", expect.objectContaining({
    request: expect.objectContaining({protocol: expect.objectContaining({
      document: "mutation Increment { add(value:2) }", operation_name: "Increment",
    })}),
  }));
});
it("requires selection when a stale name cannot disambiguate multiple operations", async () => {
  await expect(createRustGraphQLFetcher(async () => context())({
    query: "query First {value} query Second {value}", operationName: "Removed",
  })).rejects.toThrow("请选择");
  expect(api).not.toHaveBeenCalled();
});
