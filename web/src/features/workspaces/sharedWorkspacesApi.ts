import { api } from "../../shared/api";
import type { SharedWorkspacePage, SharedWorkspaceSummary } from "../../shared/types";

export async function fetchSharedWorkspaces(): Promise<SharedWorkspaceSummary[]> {
  const workspaces: SharedWorkspaceSummary[] = [];
  const seen = new Set<string>();
  let cursor: string | null = null;
  for (;;) {
    const path: string = `/api/workspaces/shared${cursor ? `?cursor=${encodeURIComponent(cursor)}` : ""}`;
    const page: SharedWorkspacePage = await api<SharedWorkspacePage>(path);
    if (!Array.isArray(page.items)) throw new Error("Invalid shared workspace page");
    workspaces.push(...page.items);
    if (!page.next_cursor) return workspaces;
    if (seen.has(page.next_cursor)) throw new Error("Repeated shared workspace cursor");
    seen.add(page.next_cursor);
    cursor = page.next_cursor;
  }
}
