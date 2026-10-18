import { beforeEach, expect, it, vi } from "vitest";
import { api } from "../../shared/api";
import { fetchSharedWorkspaces } from "./sharedWorkspacesApi";

vi.mock("../../shared/api", () => ({ api: vi.fn() }));
beforeEach(() => { vi.mocked(api).mockReset(); });

it("loads every shared workspace page for the picker", async () => {
  const first = Array.from({ length: 100 }, (_, index) => ({ owner_id: "owner", owner_username: "owner", workspace_id: `w${index}`, workspace_name: `Workspace ${index}`, revision: 1, role: "viewer" }));
  vi.mocked(api).mockImplementation((path) => Promise.resolve(path.includes("cursor=") ? { items: [{ ...first[0], workspace_id: "older" }], next_cursor: null } : { items: first, next_cursor: "next-page" }));
  const all = await fetchSharedWorkspaces();
  expect(all).toHaveLength(101);
  expect(all[100].workspace_id).toBe("older");
  expect(api).toHaveBeenCalledWith("/api/workspaces/shared?cursor=next-page");
});
