import { beforeEach, expect, it, vi } from "vitest";
import { api, token } from "../../shared/api";
import { fetchSharedWorkspaces } from "./sharedWorkspacesApi";

vi.mock("../../shared/api", () => ({ native: false, token: vi.fn(() => "member"), api: vi.fn() }));
beforeEach(() => { vi.mocked(api).mockReset(); vi.mocked(token).mockReturnValue("member"); });

it("loads every shared workspace page for the picker", async () => {
  const first = Array.from({ length: 100 }, (_, index) => ({ owner_id: "owner", owner_username: "owner", workspace_id: `w${index}`, workspace_name: `Workspace ${index}`, revision: 1, role: "viewer" }));
  vi.mocked(api).mockImplementation((path) => Promise.resolve(path.includes("cursor=") ? { items: [{ ...first[0], workspace_id: "older" }], next_cursor: null } : { items: first, next_cursor: "next-page" }));
  const all = await fetchSharedWorkspaces();
  expect(all).toHaveLength(101);
  expect(all[100].workspace_id).toBe("older");
  expect(api).toHaveBeenCalledWith("/api/workspaces/shared?cursor=next-page");
});

it("does not send a previous account cursor under a changed member credential",async()=>{vi.mocked(api).mockImplementationOnce(async()=>{vi.mocked(token).mockReturnValue("another-member");return {items:[],next_cursor:"private-old-cursor"};});await expect(fetchSharedWorkspaces()).rejects.toThrow();expect(api).toHaveBeenCalledTimes(1);});
