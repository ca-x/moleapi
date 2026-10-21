import { memberApi as api, memberCredential } from "../../shared/memberApi";
import { LocalizedError } from "../../shared/i18n/errors";
import type { SharedWorkspacePage, SharedWorkspaceSummary } from "../../shared/types";

export async function fetchSharedWorkspaces(): Promise<SharedWorkspaceSummary[]> {
  const credential = memberCredential();
  const ensureCurrent = () => { if (memberCredential() !== credential) throw new LocalizedError("共享工作区列表已过期，请重新加载。"); };
  const workspaces: SharedWorkspaceSummary[] = [];
  const seen = new Set<string>();
  let cursor: string | null = null;
  for (;;) {
    ensureCurrent();
    const path: string = `/api/workspaces/shared${cursor ? `?cursor=${encodeURIComponent(cursor)}` : ""}`;
    const page: SharedWorkspacePage = await api<SharedWorkspacePage>(path);
    ensureCurrent();
    if (!Array.isArray(page.items)) throw new Error("Invalid shared workspace page");
    workspaces.push(...page.items);
    if (!page.next_cursor) return workspaces;
    if (seen.has(page.next_cursor)) throw new Error("Repeated shared workspace cursor");
    seen.add(page.next_cursor);
    cursor = page.next_cursor;
  }
}
