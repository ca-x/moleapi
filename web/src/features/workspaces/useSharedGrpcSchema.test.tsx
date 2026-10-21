// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { api } from "../../shared/api";
import { initialData, newRequest } from "../../shared/model";
import { setLanguage } from "../../shared/i18n";
import type { Specification, Workspace } from "../../shared/types";
import { useSharedGrpcSchema, type SharedGrpcSourceContext } from "./useSharedGrpcSchema";
vi.mock("../../shared/api", () => ({ native: false,  api: vi.fn() }));
const base = "/api/owners/owner/workspaces/w";
const spec: Specification = { id: "proto", name: "Reflected", kind: "protobuf", dialect: "protobuf", source: '{"kind":"descriptor","descriptor_set_base64":"AA=="}' };
const schema = { services: [] };
function context(attach = vi.fn().mockReturnValue("proto")): SharedGrpcSourceContext {
  const data = initialData(); const request = { ...newRequest(), id: "r", method: "POST", protocol: { kind: "grpc" as const, service: "", method: "", message_source: "{}" } };
  data.collections[0].requests = [request];
  const saved: Workspace = { id: "w", name: "Shared", revision: 1, updated_at: "now", data };
  return { scope: "member", base, request, saved, data, dirty: false, saving: false, conflict: false, isCurrent: () => true, onAttach: attach };
}
beforeEach(async () => { await setLanguage("zh-CN"); vi.mocked(api).mockReset(); });
afterEach(cleanup);
it("reflects the canonical member request and stages a definition without saving or applying response updates", async () => {
  const attach = vi.fn().mockReturnValue("proto"); const props = context(attach), original = structuredClone(props.saved.data);
  vi.mocked(api).mockResolvedValue({ specification: spec, schema, variable_updates: [{ scope: "project", key: "private", value: "do-not-apply" }] });
  const { result } = renderHook(() => useSharedGrpcSchema(props));
  await act(async () => result.current.reflect());
  expect(api).toHaveBeenCalledWith(`${base}/grpc/reflect`, "POST", { request_id: "r", expected_revision: 1 });
  expect(attach).toHaveBeenCalledWith(spec); expect(props.saved.data).toEqual(original);
  expect(vi.mocked(api).mock.calls.some(([, method]) => method === "PUT")).toBe(false);
});
it.each(["request", "scope", "revision", "dirty", "conflict", "admission"] as const)("withholds delayed Reflection attachment after %s changes", async (boundary) => {
  const attach = vi.fn().mockReturnValue("proto"); let props = context(attach), admitted = true;
  props.isCurrent = () => admitted;
  let finish!: (value: unknown) => void; vi.mocked(api).mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
  const { result, rerender } = renderHook((input) => useSharedGrpcSchema(input), { initialProps: props });
  act(() => { void result.current.reflect(); });
  if (boundary === "admission") admitted = false;
  else { props = { ...props, ...(boundary === "request" ? { request: { ...props.request, id: "other" } } : {}), ...(boundary === "scope" ? { scope: "other" } : {}), ...(boundary === "revision" ? { saved: { ...props.saved, revision: 2 } } : {}), ...(boundary === "dirty" ? { dirty: true } : {}), ...(boundary === "conflict" ? { conflict: true } : {}) }; rerender(props); }
  await act(async () => finish({ specification: spec, schema }));
  expect(attach).not.toHaveBeenCalled(); expect(result.current.busy).toBe(false);
});
it("requires a clean saved draft before importing or reflecting", async () => {
  const props = { ...context(), dirty: true }; const { result } = renderHook(() => useSharedGrpcSchema(props));
  await act(async () => result.current.reflect());
  await expect(result.current.importSource("Proto", "{}" )).rejects.toThrow("先保存");
  expect(api).not.toHaveBeenCalled();
});
it("loads only the saved request-linked schema through the member route", async () => {
  const props = context(); props.request = { ...props.request, specification_id: "proto" }; props.data.specifications = [spec];
  vi.mocked(api).mockResolvedValue({ specification: spec, schema });
  const { result } = renderHook(() => useSharedGrpcSchema(props));
  await waitFor(() => expect(result.current.schema).toEqual(schema));
  expect(api).toHaveBeenCalledWith(`${base}/grpc/schema`, "POST", { request_id: "r", specification_id: "proto", expected_revision: 1 });
});

it("does not retarget retained source actions into a newly rendered scope", async () => {
  const attach = vi.fn().mockReturnValue("proto"); const props = context(attach);
  vi.mocked(api).mockResolvedValue({ specification: spec, schema });
  const { result, rerender } = renderHook((input) => useSharedGrpcSchema(input), { initialProps: props });
  const retained = result.current;
  rerender({ ...props, scope: "new-account" });
  await act(async () => { retained.attach(spec, schema); await retained.reflect(); });
  expect(attach).not.toHaveBeenCalled(); expect(api).not.toHaveBeenCalled();
});

it("reloads the saved schema after edits are reverted while an earlier schema read was withheld", async () => {
  const props = context(); props.request = { ...props.request, specification_id: "proto" }; props.data.specifications = [spec];
  let finish!: (value: unknown) => void;
  vi.mocked(api).mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; })).mockResolvedValue({ specification: spec, schema });
  const { result, rerender } = renderHook((input) => useSharedGrpcSchema(input), { initialProps: props });
  rerender({ ...props, dirty: true });
  await act(async () => finish({ specification: spec, schema }));
  expect(result.current.schema).toBeNull();
  rerender(props);
  await waitFor(() => expect(result.current.schema).toEqual(schema));
  expect(api).toHaveBeenCalledTimes(2);
});
