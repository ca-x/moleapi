import { toast } from "sonner";
import { useState } from "react";
import { native } from "../../shared/api";
import { id } from "../../shared/model";
import type { Pair, VariableUpdate, Workspace } from "../../shared/types";
import type { useWorkspace } from "../workspaces/useWorkspace";
import {
  assignLocal,
  bucketKey,
  cleanBuckets,
  executionLocals,
  reconcileLocalRows,
} from "./localValues";
function load(key: string) {
  try {
    return cleanBuckets(JSON.parse(localStorage.getItem(key) || "{}"));
  } catch {
    return {};
  }
}
export function useLocalVariables(workspace: ReturnType<typeof useWorkspace>) {
  const key = `moleapi:local-values:${encodeURIComponent(workspace.accountId)}:${encodeURIComponent(workspace.draft?.id || "")}`;
  const [stored, setStored] = useState(() => ({ key, buckets: load(key) }));
  const buckets = stored.key === key ? stored.buckets : load(key);
  const findPairs = (scope: string, target: string) =>
    scope === "project"
      ? workspace.draft?.data.global_variables || []
      : scope === "collection"
        ? workspace.draft?.data.collections.find((c) => c.id === target)
            ?.variables || []
        : workspace.draft?.data.environments.find((e) => e.id === target)
            ?.variables || [];
  function read(
    scope: string,
    target: string,
    name: string,
    rowId?: string,
  ): string | undefined {
    if (native)
      return (
        findPairs(scope, target).find((p) =>
          rowId ? p.id === rowId : p.key === name,
        )?.local_value ?? undefined
      );
    const entries = buckets[bucketKey(scope, target)] || {};
    return Object.hasOwn(entries, name)
      ? entries[name]
      : rowId
        ? buckets[bucketKey(`editing:${scope}`, target)]?.[rowId]
        : undefined;
  }
  function write(
    scope: string,
    target: string,
    name: string,
    value: string | undefined,
    rowId?: string,
  ) {
    if (!name && !rowId) return;
    if (native) {
      workspace.updateData((data) => {
        const update = (pairs: Pair[]) => {
          const matches = (p: Pair) =>
            rowId ? p.id === rowId : p.key === name;
          const existing = pairs.find(matches);
          if (rowId && !existing) return pairs;
          if (!existing && value !== undefined)
            return [
              ...pairs,
              {
                id: id(),
                key: name,
                value: "",
                enabled: true,
                secret: true,
                local_value: value,
              },
            ];
          return pairs.map((p) =>
            matches(p) ? { ...p, local_value: value } : p,
          );
        };
        if (scope === "project")
          return {
            ...data,
            global_variables: update(data.global_variables || []),
          };
        if (scope === "collection")
          return {
            ...data,
            collections: data.collections.map((c) =>
              c.id === target
                ? { ...c, variables: update(c.variables || []) }
                : c,
            ),
          };
        return {
          ...data,
          environments: data.environments.map((e) =>
            e.id === target ? { ...e, variables: update(e.variables) } : e,
          ),
        };
      });
      return;
    }
    const next = assignLocal(
      load(key),
      name ? scope : `editing:${scope}`,
      target,
      name || rowId!,
      value,
    );
    persist(next);
  }
  function persist(next: ReturnType<typeof load>) {
    try {
      localStorage.setItem(key, JSON.stringify(next));
      setStored({ key, buckets: next });
      return true;
    } catch {
      toast.error("无法保存本地变量，请检查浏览器存储权限或剩余空间。");
      return false;
    }
  }
  function values(
    snapshot: Workspace,
    collectionId: string | undefined,
    environmentId: string | null,
  ) {
    return executionLocals(
      snapshot,
      collectionId,
      environmentId,
      buckets,
      native,
    );
  }
  function apply(
    snapshot: Workspace,
    collectionId: string | undefined,
    environmentId: string | null,
    updates: VariableUpdate[],
  ) {
    if (
      workspace.accountRef.current !== workspace.accountId ||
      workspace.stateRef.current.draft?.id !== snapshot.id
    )
      return;
    for (const update of updates) {
      if (update.scope === "project")
        write("project", "", update.key, update.value);
      else if (update.scope === "collection" && collectionId)
        write("collection", collectionId, update.key, update.value);
      else if (update.scope === "environment" && environmentId)
        write("environment", environmentId, update.key, update.value);
    }
  }
  const entries = (scope: string, target: string) =>
    native
      ? Object.fromEntries(
          findPairs(scope, target)
            .filter(
              (p) => p.local_value !== undefined && p.local_value !== null,
            )
            .map((p) => [p.key, p.local_value!]),
        )
      : buckets[bucketKey(scope, target)] || {};
  function reconcile(
    scope: string,
    target: string,
    before: Pair[],
    after: Pair[],
  ) {
    if (native) return true;
    const current = load(key);
    const next = reconcileLocalRows(current, scope, target, before, after);
    return next === current || persist(next);
  }

  return { read, write, values, apply, entries, reconcile };
}
