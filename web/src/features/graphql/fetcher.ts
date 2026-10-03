import { getOperationAST, parse } from "graphql";
import type { Fetcher, FetcherParams } from "@graphiql/toolkit";
import { api } from "../../shared/api";
import type {
  ApiResponse,
  RequestSpec,
  VariableUpdate,
} from "../../shared/types";
import type { EventBatch, ProtocolSession } from "../protocols/types";
export interface GraphQLContext {
  workspace_id: string;
  request: RequestSpec;
  environment_id: string | null;
  locals: VariableUpdate[];
  current: () => boolean;
  apply: (updates: VariableUpdate[]) => void;
  response: (response: ApiResponse) => void;
  sessions: Set<string>;
}
function executionInput(context: GraphQLContext, parameters: FetcherParams) {
  if (context.request.protocol?.kind !== "graphql")
    throw new Error("当前请求不是 GraphQL");
  const variables = parameters.variables ?? {};
  if (typeof variables !== "object" || Array.isArray(variables))
    throw new Error("GraphQL 变量必须为 JSON 对象");
  const request = {
    ...structuredClone(context.request),
    protocol: {
      ...context.request.protocol,
      document: parameters.query,
      variables,
      variables_source: JSON.stringify(variables),
      operation_name: parameters.operationName ?? null,
    },
  };
  return {
    workspace_id: context.workspace_id,
    request,
    environment_id: context.environment_id,
    ...(context.locals.length ? { locals: context.locals } : {}),
  };
}
function payload(response: ApiResponse) {
  let result: unknown;
  try {
    result = JSON.parse(response.body);
  } catch {
    throw new Error(`GraphQL 服务返回非 JSON 内容（HTTP ${response.status}）`);
  }
  if (!result || typeof result !== "object" || Array.isArray(result))
    throw new Error("GraphQL 服务响应必须为 JSON 对象");
  if (!Object.hasOwn(result, "data") && !Object.hasOwn(result, "errors"))
    throw new Error(
      `服务未返回 GraphQL data/errors（HTTP ${response.status}）`,
    );
  return result as { data?: unknown; errors?: unknown[] };
}
export function createRustGraphQLFetcher(
  getContext: () => Promise<GraphQLContext>,
): Fetcher {
  return async (parameters) => {
    const context = await getContext();
    if (!context.current()) throw new Error("GraphQL 请求已取消");
    // GraphiQL refreshes operation facts after a debounce; execution must use
    // the actual editor text, including Send immediately after an edit.
    const document = parse(parameters.query);
    const operation = getOperationAST(document, parameters.operationName)
      ?? getOperationAST(document);

    if (!operation) throw new Error("请选择要执行的 GraphQL operation");
    const input = executionInput(context, { ...parameters, operationName: operation.name?.value });
    if (operation.operation !== "subscription") {
      const response = await api<ApiResponse>("/api/execute", "POST", input);
      if (!context.current()) throw new Error("GraphQL 请求已取消");
      context.apply(response.variable_updates || []);
      context.response(response);
      return payload(response);
    }
    return cancellableSubscription(context, input);
  };
}
function cancellableSubscription(
  context: GraphQLContext,
  input: ReturnType<typeof executionInput>,
) {
  let cancelled = false;
  let sessionId: string | undefined;
  const active = { ...context, current: () => !cancelled && context.current() };
  const generator = subscribe(active, input, (id) => {
    sessionId = id;
  });
  const close = () => {
    if (sessionId)
      void api(
        `/api/sessions/${encodeURIComponent(sessionId)}/close`,
        "POST",
        {},
      ).catch(() => {});
  };
  const iterator: AsyncIterableIterator<unknown> = {
    next: () =>
      cancelled
        ? Promise.resolve({ done: true, value: undefined })
        : generator.next(),
    return: () => {
      cancelled = true;
      close();
      void generator.return(undefined).catch(() => {});
      return Promise.resolve({ done: true, value: undefined });
    },
    [Symbol.asyncIterator]() {
      return iterator;
    },
  };
  return iterator;
}
async function* subscribe(
  context: GraphQLContext,
  input: ReturnType<typeof executionInput>,
  created: (id: string) => void,
) {
  const session = await api<ProtocolSession>("/api/sessions", "POST", input);
  created(session.id);
  context.sessions.add(session.id);
  let cursor = 0;
  try {
    if (!context.current()) return;
    context.apply(session.variable_updates || []);
    while (context.current()) {
      const state = await api<ProtocolSession>(
        `/api/sessions/${encodeURIComponent(session.id)}`,
      );
      if (!context.current()) return;
      const batch = await api<EventBatch>(
        `/api/sessions/${encodeURIComponent(session.id)}/events?after=${cursor}`,
      );
      if (!context.current()) return;
      if (batch.dropped_count)
        throw new Error(
          `GraphQL 订阅已丢失 ${batch.dropped_count} 条事件，请重新订阅`,
        );
      cursor = batch.next_cursor;
      for (const event of batch.events) {
        if (event.message.kind === "graphql_next") {
          const value = event.message.payload;
          if (!value || typeof value !== "object" || Array.isArray(value))
            throw new Error("GraphQL 订阅返回无效 payload");
          yield value;
        } else if (event.message.kind === "graphql_error") {
          yield {
            errors: Array.isArray(event.message.payload)
              ? event.message.payload
              : [
                  {
                    message: "GraphQL 订阅失败",
                    extensions: { detail: event.message.payload },
                  },
                ],
          };
          return;
        } else if (event.message.kind === "graphql_complete") return;
      }
      if (state.state === "error")
        throw new Error(state.reason || "GraphQL 订阅连接失败");
      if (state.state === "closed") return;
      await new Promise((resolve) => setTimeout(resolve, 500));
    }
  } finally {
    context.sessions.delete(session.id);
    await api(
      `/api/sessions/${encodeURIComponent(session.id)}/close`,
      "POST",
      {},
    ).catch(() => {});
  }
}
