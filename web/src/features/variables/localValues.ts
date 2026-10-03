import type { Pair, VariableUpdate, Workspace } from "../../shared/types";
export type LocalBuckets = Record<string, Record<string, string>>;
export const bucketKey = (scope: string, target = "") =>
  JSON.stringify([scope, target]);
export function cleanBuckets(value: unknown): LocalBuckets {
  const output: LocalBuckets = {};
  if (!value || typeof value !== "object" || Array.isArray(value))
    return output;
  for (const [scope, entries] of Object.entries(value)) {
    if (!entries || typeof entries !== "object" || Array.isArray(entries))
      continue;
    output[scope] = Object.fromEntries(
      Object.entries(entries).filter(
        ([key, value]) =>
          key.length <= 1024 &&
          typeof value === "string" &&
          value.length <= 1024 * 1024,
      ),
    ) as Record<string, string>;
  }
  return output;
}
export function executionLocals(
  workspace: Workspace,
  collectionId: string | undefined,
  environmentId: string | null,
  buckets: LocalBuckets,
  isNative: boolean,
): VariableUpdate[] {
  const result = new Map<string, VariableUpdate>();
  const scopes: [string, string, Pair[]][] = [
    ["project", "", workspace.data.global_variables || []],
    [
      "collection",
      collectionId || "",
      workspace.data.collections.find((c) => c.id === collectionId)
        ?.variables || [],
    ],
    [
      "environment",
      environmentId || "",
      workspace.data.environments.find((e) => e.id === environmentId)
        ?.variables || [],
    ],
  ];
  for (const [scope, target, pairs] of scopes) {
    if (
      (scope === "environment" && !environmentId) ||
      (scope === "collection" && !collectionId)
    )
      continue;
    if (isNative)
      for (const pair of pairs)
        if (
          pair.enabled &&
          pair.local_value !== undefined &&
          pair.local_value !== null
        )
          result.set(`${scope}\0${pair.key}`, {
            scope,
            key: pair.key,
            value: pair.local_value,
          });
    const stored = buckets[bucketKey(scope, target)] || {};
    for (const [key, value] of Object.entries(stored)) {
      if (pairs.some((p) => p.key === key && !p.enabled)) continue;
      result.set(`${scope}\0${key}`, { scope, key, value });
    }
  }
  return [...result.values()];
}
export function assignLocal(
  buckets: LocalBuckets,
  scope: string,
  target: string,
  key: string,
  value: string | undefined,
): LocalBuckets {
  const id = bucketKey(scope, target);
  const entries = { ...(buckets[id] || {}) };
  if (value === undefined) delete entries[key];
  else
    Object.defineProperty(entries, key, {
      value,
      writable: true,
      enumerable: true,
      configurable: true,
    });
  return { ...buckets, [id]: entries };
}

export function reconcileLocalRows(
  buckets: LocalBuckets,
  scope: string,
  target: string,
  before: Pair[],
  after: Pair[],
): LocalBuckets {
  const entries = buckets[bucketKey(scope, target)] || {};
  const editingScope = `editing:${scope}`;
  const editing = buckets[bucketKey(editingScope, target)] || {};
  let next = buckets;
  for (const row of before) {
    const updated = after.find((candidate) => candidate.id === row.id);
    if (updated?.key === row.key) continue;
    const value = Object.hasOwn(entries, row.key)
      ? entries[row.key]
      : editing[row.id];
    if (value === undefined) continue;
    next = assignLocal(next, scope, target, row.key, undefined);
    next = assignLocal(next, editingScope, target, row.id, undefined);
    if (updated)
      next = updated.key
        ? assignLocal(next, scope, target, updated.key, value)
        : assignLocal(next, editingScope, target, row.id, value);
  }
  return next;
}
