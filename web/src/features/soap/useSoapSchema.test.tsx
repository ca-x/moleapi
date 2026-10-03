// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { api } from "../../shared/api";
import { initialData, newRequest } from "../../shared/model";
import { useWorkbench } from "../workbench/context";
import { useSoapSchema } from "./useSoapSchema";
vi.mock("../../shared/api", () => ({ api: vi.fn() }));
vi.mock("../workbench/context", () => ({ useWorkbench: vi.fn() }));
const specification = {
  id: "wsdl",
  name: "Service",
  kind: "wsdl",
  dialect: "wsdl1.1",
  source: "original",
};
const schema = { services: [{ name: "Service", ports: [] }] };
beforeEach(() => {
  vi.mocked(api).mockReset();
});
function setup() {
  const data = initialData();
  data.specifications = [specification];
  const state = {
    authenticated: true,
    accountId: "account",
    draft: { id: "workspace", data },
    request: {
      ...newRequest(),
      id: "request",
      specification_id: "wsdl",
      protocol: {
        kind: "soap" as const,
        version: "1.1" as const,
        service: "",
        port: "",
        operation: "",
        action: "",
      },
    },
    dirty: false,
    save: vi.fn().mockResolvedValue(true),
    updateData: vi.fn(),
    updateRequest: vi.fn(),
  };
  vi.mocked(useWorkbench).mockImplementation(
    () => state as unknown as ReturnType<typeof useWorkbench>,
  );
  return state;
}
it("discarding a delayed schema prevents old workspace data entering a newly selected owner", async () => {
  const state = setup();
  const resolvers: ((value: unknown) => void)[] = [];
  vi.mocked(api).mockImplementation(
    () =>
      new Promise((done) => {
        resolvers.push(done);
      }),
  );
  const hook = renderHook(useSoapSchema);
  state.accountId = "other";
  await act(async () => {
    hook.rerender();
  });
  const outgoing = resolvers[0];
  await act(async () => {
    outgoing({ specification, schema });
  });
  expect(hook.result.current.schema).toBeNull();
  expect(state.updateRequest).not.toHaveBeenCalled();
});
it("a schema lookup reads the canonical source without calling external WSDL URL import", async () => {
  setup();
  vi.mocked(api).mockResolvedValue({ specification, schema });
  const hook = renderHook(useSoapSchema);
  await act(async () => {
    await Promise.resolve();
  });
  expect(hook.result.current.schema).toEqual(schema);
  expect(api).toHaveBeenCalledWith("/api/soap/schema", "POST", {
    workspace_id: "workspace",
    specification_id: "wsdl",
  });
  expect(api).not.toHaveBeenCalledWith(
    "/api/soap/import-url",
    expect.anything(),
    expect.anything(),
  );
});

it("attaching a new definition clears operation selection from the previous WSDL", async () => {
  const state = setup();
  state.request.protocol = { ...state.request.protocol, service: "Previous", port: "Old", operation: "Echo", action: "OldAction" };
  vi.mocked(api).mockResolvedValue({ specification, schema });
  const hook = renderHook(useSoapSchema);
  await act(async () => { await Promise.resolve(); });
  await act(async () => { hook.result.current.attach({ ...specification, id: "new", source: "new source" }, schema); });
  expect(state.updateRequest).toHaveBeenCalledWith(expect.objectContaining({
    specification_id: "new", protocol: expect.objectContaining({service: "", port: "", operation: "", action: ""}),
  }));
});
