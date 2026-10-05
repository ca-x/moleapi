import type { Collection, WorkspaceData } from "../../shared/types";
import { LocalizedError } from "../../shared/i18n/errors";
export const collectionDepthLimit = 16;
export function collectionDescendants(collections: Collection[], id: string) {
  const ids = new Set<string>();
  const pending = [id];
  while (pending.length) {
    const current = pending.pop()!;
    if (ids.has(current)) continue;
    ids.add(current);
    pending.push(...collections.filter(c => c.parent_id === current).map(c => c.id));
  }
  return ids;
}
export function collectionAncestors(collections: Collection[], id: string) {
  const byId = new Map(collections.map(c => [c.id, c]));
  const path: Collection[] = [];
  let current = byId.get(id);
  const seen = new Set<string>();
  while (current) {
    if (seen.has(current.id) || path.length >= collectionDepthLimit) throw new LocalizedError("目录层级无效或超过限制");
    seen.add(current.id); path.push(current);
    if (!current.parent_id) break;
    current = byId.get(current.parent_id);
    if (!current) throw new LocalizedError("找不到父目录");
  }
  return path.reverse();
}
export function collectionRows(collections: Collection[], collapsed: Set<string> = new Set()) {
  const rows: { collection: Collection; depth: number }[] = [];
  const seen = new Set<string>();
  const visit = (parent: string | null, depth: number) => {
    if (depth >= collectionDepthLimit) return;
    for (const collection of collections.filter(c => (c.parent_id ?? null) === parent)) {
      if (seen.has(collection.id)) continue;
      seen.add(collection.id); rows.push({ collection, depth });
      if (!collapsed.has(collection.id)) visit(collection.id, depth + 1);
    }
  };
  visit(null, 0);
  return rows;
}
export function moveCollection(data: WorkspaceData, id: string, parent_id: string | null) {
  if (!data.collections.some(c => c.id === id)) throw new LocalizedError("找不到目录");
  if (parent_id && (!data.collections.some(c => c.id === parent_id) || collectionDescendants(data.collections, id).has(parent_id))) throw new LocalizedError("目录不能移动到自身或子目录");
  const next = { ...data, collections: data.collections.map(c => c.id === id ? { ...c, parent_id } : c) };
  for (const c of next.collections) collectionAncestors(next.collections, c.id);
  return next;
}
export function removeCollectionTree(data: WorkspaceData, id: string) {
  const ids = collectionDescendants(data.collections, id);
  return { ...data, collections: data.collections.filter(c => !ids.has(c.id)) };
}
