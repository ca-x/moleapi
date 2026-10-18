import { expect, it } from "vitest";
import { initialData } from "../../shared/model";
import type { SharedWorkspaceSummary, Workspace } from "../../shared/types";
import { ownedWorkspaceKey, resolveWorkspaceChoice, sharedWorkspaceKey } from "./workspaceSelection";

it("keeps valid owned IDs distinct from shared picker keys", () => {
  const owned: Workspace = { id: "shared:owner:w", name: "Owned", revision: 1, updated_at: "now", data: initialData() };
  const shared: SharedWorkspaceSummary = { owner_id: "owner", owner_username: "owner", workspace_id: "w", workspace_name: "Shared", revision: 1, role: "viewer" };
  const ownKey = ownedWorkspaceKey(owned.id);
  const memberKey = sharedWorkspaceKey(shared.owner_id, shared.workspace_id);
  expect(ownKey).not.toBe(memberKey);
  expect(resolveWorkspaceChoice(ownKey, [owned], [shared])).toEqual({ kind: "owned", workspace: owned });
  expect(resolveWorkspaceChoice(memberKey, [owned], [shared])).toEqual({ kind: "shared", workspace: shared });
});
