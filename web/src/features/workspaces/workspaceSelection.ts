import type { SharedWorkspaceSummary, Workspace } from "../../shared/types";

export const ownedWorkspaceKey = (id: string) => `owned:${encodeURIComponent(id)}`;
export const sharedWorkspaceKey = (ownerId: string, workspaceId: string) => `shared:${encodeURIComponent(ownerId)}:${encodeURIComponent(workspaceId)}`;

export function resolveWorkspaceChoice(value: string, owned: Workspace[], shared: SharedWorkspaceSummary[]) {
  if (value === "none") return { kind: "none" as const };
  if (value.startsWith("owned:")) {
    const workspace = owned.find((item) => ownedWorkspaceKey(item.id) === value);
    return workspace ? { kind: "owned" as const, workspace } : null;
  }
  if (value.startsWith("shared:")) {
    const workspace = shared.find((item) => sharedWorkspaceKey(item.owner_id, item.workspace_id) === value);
    return workspace ? { kind: "shared" as const, workspace } : null;
  }
  // Existing controller callers may still pass an unprefixed owned ID.
  const workspace = owned.find((item) => item.id === value);
  return workspace ? { kind: "owned" as const, workspace } : null;
}
