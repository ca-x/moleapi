// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { api } from "../../shared/api";
import { initialData, newRequest } from "../../shared/model";
import { useWorkbench } from "../workbench/context";
import { useGrpcSchema } from "./useGrpcSchema";
vi.mock("../../shared/api", () => ({ api: vi.fn(), native: false }));
vi.mock("../workbench/context", () => ({ useWorkbench: vi.fn() }));
const spec = {
  id: "schema",
  name: "Fixture",
  kind: "protobuf",
  source: '{"kind":"descriptor","descriptor_set_base64":""}',
  dialect: "proto3",
};
const schema = { services: [{ name: "Fixture", methods: [] }] };
function setup() {
  const request = {
    ...newRequest(),
    id: "request",
    protocol: {
      kind: "grpc" as const,
      service: "",
      method: "",
      message_source: "{}",
    },
  };
  const draft = {
    id: "workspace",
    name: "Fixture",
    revision: 1,
    updated_at: "now",
    data: initialData(),
  };
  draft.data.collections[0].requests = [request];
  const state = {
    authenticated: true,
    accountId: "account",
    draft,
    request,
    dirty: false,
    save: vi.fn().mockResolvedValue(true),
    updateData: vi.fn(),
    updateRequest: vi.fn(),
    localVariables: {
      values: vi
        .fn()
        .mockReturnValue([
          { scope: "environment", key: "token", value: "private" },
        ]),
      apply: vi.fn(),
    },
  };
  vi.mocked(useWorkbench).mockImplementation(
    () => state as unknown as ReturnType<typeof useWorkbench>,
  );
  return state;
}
beforeEach(() => {
  vi.mocked(api).mockReset();
});
it("keeps target reflection status separate from MoleAPI authentication and never attaches a failed schema", async () => {
  const state = setup();
  vi.mocked(api).mockResolvedValue({
    status: {
      code: 16,
      name: "UNAUTHENTICATED",
      message: "Authentication required",
      details_base64: "",
    },
  });
  const { result } = renderHook(useGrpcSchema);
  await act(async () => {
    await result.current.reflect();
  });
  expect(result.current.error).toContain("16 UNAUTHENTICATED");
  expect(state.authenticated).toBe(true);
  expect(state.updateData).not.toHaveBeenCalled();
  expect(state.updateRequest).not.toHaveBeenCalled();
  expect(api).toHaveBeenCalledWith(
    "/api/grpc/reflect",
    "POST",
    expect.objectContaining({
      workspace_id: "workspace",
      locals: [{ scope: "environment", key: "token", value: "private" }],
    }),
  );
});
it("discards late reflection schemas and variable updates after switching environment", async () => {
  const state = setup();
  let resolve!: (value: unknown) => void;
  vi.mocked(api).mockImplementation(
    () =>
      new Promise((done) => {
        resolve = done;
      }),
  );
  const { result, rerender } = renderHook(useGrpcSchema);
  let work!: Promise<void>;
  act(() => {
    work = result.current.reflect();
  });
  state.draft.data.active_environment_id = "production";
  rerender();
  await act(async () => {
    resolve({
      specification: spec,
      schema,
      variable_updates: [{ scope: "environment", key: "token", value: "late" }],
    });
    await work;
  });
  expect(state.localVariables.apply).not.toHaveBeenCalled();
  expect(state.updateRequest).not.toHaveBeenCalled();
  expect(result.current.schema).toBeNull();
});
it("appends a new source without deleting definitions used by other requests", async () => {
  const state = setup();
  const old = { ...spec, id: "previous", source: "previous source" };
  state.draft.data.specifications = [old];
  vi.mocked(api).mockResolvedValue({ specification: spec, schema });
  const { result } = renderHook(useGrpcSchema);
  await act(async () => {
    await result.current.reflect();
  });
  const mutation = state.updateData.mock.calls[0][0] as unknown as (
    data: typeof state.draft.data,
  ) => typeof state.draft.data;
  expect(mutation(state.draft.data).specifications).toEqual([old, spec]);
  expect(state.updateRequest).toHaveBeenCalledWith({
    specification_id: "schema",
  });
});
it("switches a caught local save failure without repeating reflection or rewriting the request", async () => {
  const { setLanguage } = await import("../../shared/i18n");
  const state = setup(); state.dirty = true; state.save.mockResolvedValue(false);
  const original = JSON.stringify(state.request);
  const { result } = renderHook(useGrpcSchema);
  await act(async () => { await result.current.reflect(); });
  expect(result.current.error).toBe("请先解决工作区保存冲突");
  await act(async () => { await setLanguage("en"); });
  expect(result.current.error).toBe("Resolve the workspace save conflict first");
  expect(state.save).toHaveBeenCalledTimes(1);
  expect(api).not.toHaveBeenCalled();
  expect(JSON.stringify(state.request)).toBe(original);
});
