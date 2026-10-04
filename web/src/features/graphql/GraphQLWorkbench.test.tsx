// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { api } from "../../shared/api";
import { initialData, newRequest } from "../../shared/model";
import { setLanguage } from "../../shared/i18n";
import { useWorkbench } from "../workbench/context";
import GraphQLWorkbench from "./GraphQLWorkbench";
vi.mock("../../shared/api", () => ({ api: vi.fn() }));
vi.mock("../workbench/context", () => ({ useWorkbench: vi.fn() }));
vi.mock("graphiql/setup-workers/vite", () => ({}));
// Keep this regression about the workbench error lifecycle, not Monaco layout.
vi.mock("graphiql", () => ({ GraphiQL: ({ initialQuery, initialVariables }: { initialQuery: string; initialVariables: string }) => <><textarea aria-label="Query draft" readOnly value={initialQuery} /><textarea aria-label="Variable draft" readOnly value={initialVariables} /></> }));
vi.mock("../requests/ResponsePane", () => ({ ResponsePane: () => null }));
let state: ReturnType<typeof useWorkbench>;
beforeEach(() => {
  vi.mocked(api).mockReset();
  const request = { ...newRequest("我的 GraphQL", "https://example.test/graphql"), id: "r", protocol: { kind: "graphql" as const, document: "query Original { hello } # 原文", variables: {}, connection_params: {}, variables_source: '{"原文":"值"}', operation_name: "Original" } };
  const data = initialData(); data.collections[0].requests = [request];
  state = { authenticated: true, accountId: "owner", draft: { id: "w", data }, request, dark: false, dirty: false, save: vi.fn().mockResolvedValue(true), localVariables: { values: () => [], apply: vi.fn() }, updateData: vi.fn(), updateRequest: vi.fn(), graphqlRun: { current: null } } as unknown as ReturnType<typeof useWorkbench>;
  vi.mocked(useWorkbench).mockImplementation(() => state);
});
afterEach(cleanup);
it("retains a caught schema save-conflict key through a live switch without retrying the operation", async () => {
  state.dirty = true; state.save = vi.fn().mockResolvedValue(false);
  state.request!.specification_id = "schema";
  state.draft!.data.specifications = [{ id: "schema", name: "我的定义", kind: "graphql-sdl", dialect: "graphql", source: "type Query { hello: String }" }];
  const original = JSON.stringify(state.request);
  render(<GraphQLWorkbench />);
  await waitFor(() => expect(screen.getByRole("alert").textContent).toBe("请先解决工作区保存冲突"));
  await act(async () => { await setLanguage("en"); });
  expect(screen.getByRole("alert").textContent).toBe("Resolve the workspace save conflict first");
  expect(state.save).toHaveBeenCalledTimes(1);
  expect(api).not.toHaveBeenCalled();
  expect((screen.getByLabelText("Query draft") as HTMLTextAreaElement).value).toBe(state.request!.protocol!.kind === "graphql" ? state.request!.protocol!.document : "");
  expect(JSON.stringify(state.request)).toBe(original);
});
it("switches a locally generated missing-schema failure while preserving the request and variables", async () => {
  vi.mocked(api).mockResolvedValue({ response: { status: 200, status_text: "OK", headers: [], body: "original upstream response", elapsed_ms: 1, variable_updates: [] } });
  const original = JSON.stringify(state.request);
  render(<GraphQLWorkbench />);
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "读取并保存 Schema" })); });
  expect(screen.getByRole("alert").textContent).toBe("服务未返回可用 schema");
  await act(async () => { await setLanguage("en"); });
  expect(screen.getByRole("alert").textContent).toBe("The server did not return a usable schema");
  expect(api).toHaveBeenCalledTimes(1);
  expect(JSON.stringify(state.request)).toBe(original);
  expect((screen.getByLabelText("Variable draft") as HTMLTextAreaElement).value).toBe('{"原文":"值"}');
});
it("keeps an upstream schema failure exact even when its text equals a local translation key", async () => {
  const upstream = "请先解决工作区保存冲突";
  vi.mocked(api).mockResolvedValue({ response: { status: 422, headers: [], variable_updates: [] }, error: upstream });
  render(<GraphQLWorkbench />);
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "读取并保存 Schema" })); });
  expect(screen.getByRole("alert").textContent).toBe(upstream);
  await act(async () => { await setLanguage("en"); });
  expect(screen.getByRole("alert").textContent).toBe(upstream);
  expect(api).toHaveBeenCalledTimes(1);
});
