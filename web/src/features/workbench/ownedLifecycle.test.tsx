// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Theme } from "@radix-ui/themes";
import { useState } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { api } from "../../shared/api";
import { initialData } from "../../shared/model";
import type { Workspace } from "../../shared/types";
import { useWorkspace } from "../workspaces/useWorkspace";
import { WorkbenchContext } from "./context";
import type { useWorkbenchController } from "./useWorkbenchController";
import WorkbenchMain from "./WorkbenchMain";

vi.mock("../../shared/api", async (original) => ({ ...(await original<typeof import("../../shared/api")>()), api: vi.fn() }));
vi.mock("sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));
vi.mock("../requests/RequestEditor", () => ({ default: function Editor() {
  const [draft, setDraft] = useState("");
  return <input aria-label="Protocol draft" value={draft} onChange={(event) => setDraft(event.target.value)} />;
} }));

const original: Workspace = { id: "workspace-a", name: "Original", revision: 1, updated_at: "now", data: initialData() };
let client: QueryClient;
beforeEach(() => {
  vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} });
  vi.mocked(api).mockReset();
  client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
  client.setQueryData(["workspaces", "local"], [original]);
});
afterEach(() => { cleanup(); client.clear(); vi.unstubAllGlobals(); });

function Harness() {
  const workspace = useWorkspace(true);
  const state = {
    ...workspace,
    view: "requests",
    request: workspace.draft?.data.collections[0].requests[0],
    protocolSession: { session: null },
    response: null,
    setView: vi.fn(),
    setVersionRequestId: vi.fn(),
  } as unknown as ReturnType<typeof useWorkbenchController>;
  return <WorkbenchContext.Provider value={state}>
    <button onClick={() => workspace.installWorkspace({ ...original, revision: 3 })}>Restore workspace</button>
    <button onClick={() => { workspace.setDraft((current) => current && { ...current, name: "Saved" }); void workspace.save(true); }}>Save workspace</button>
    <output aria-label="Saved revision">{workspace.saved?.revision}</output>
    <WorkbenchMain />
  </WorkbenchContext.Provider>;
}
function setup() {
  render(<QueryClientProvider client={client}><Theme><Harness /></Theme></QueryClientProvider>);
}

it("discards transient protocol drafts after restoring the same workspace and request IDs", async () => {
  setup();
  const input = await screen.findByRole("textbox", { name: "Protocol draft" });
  fireEvent.change(input, { target: { value: "unsaved private payload" } });
  fireEvent.click(screen.getByRole("button", { name: "Restore workspace" }));
  expect((screen.getByRole("textbox", { name: "Protocol draft" }) as HTMLInputElement).value).toBe("");
});

it("preserves transient editor input through ordinary workspace saves", async () => {
  vi.mocked(api).mockResolvedValue({ ...original, name: "Saved", revision: 2 });
  setup();
  const input = await screen.findByRole("textbox", { name: "Protocol draft" });
  fireEvent.change(input, { target: { value: "keep this payload" } });
  fireEvent.click(screen.getByRole("button", { name: "Save workspace" }));
  await waitFor(() => expect(screen.getByLabelText("Saved revision").textContent).toBe("2"));
  expect((screen.getByRole("textbox", { name: "Protocol draft" }) as HTMLInputElement).value).toBe("keep this payload");
});
