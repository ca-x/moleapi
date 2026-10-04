import { liveError } from "./../../shared/i18n/errors";
import { useLanguage } from "../../shared/i18n";
import { useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { api } from "../../shared/api";
import type { RunResult } from "../../shared/types";
import type { useLocalVariables } from "../variables/useLocalVariables";
import type { useWorkspace } from "../workspaces/useWorkspace";

export function useRunner(
  workspace: ReturnType<typeof useWorkspace>,
  localVariables?: ReturnType<typeof useLocalVariables>,
) {
  useLanguage();
  const { draft, dirty, save, stateRef, accountId, accountRef } = workspace;
  const client = useQueryClient();
  const [selected, setRunCollection] = useState("");
  const runCollection = draft?.data.collections.some((c) => c.id === selected)
    ? selected
    : draft?.data.collections[0]?.id || "";
  const [result, setResult] = useState<{
    accountId: string;
    workspaceId: string;
    collectionId: string;
    value: RunResult;
  } | null>(null);
  const [runnerBusy, setRunnerBusy] = useState(false);
  const running = useRef(false);
  async function run() {
    if (!draft || !runCollection || running.current) return;
    const workspaceId = draft.id;
    const collectionId = runCollection;
    const environmentId = draft.data.active_environment_id;
    running.current = true;
    setRunnerBusy(true);
    setResult(null);
    try {
      if (dirty && !(await save(true))) return;
      if (
        accountRef.current !== accountId ||
        stateRef.current.draft?.id !== workspaceId
      )
        return;
      const locals =
        localVariables?.values(draft, collectionId, environmentId) || [];
      const value = await api<RunResult>(
        `/api/workspaces/${workspaceId}/run`,
        "POST",
        {
          collection_id: collectionId,
          environment_id: environmentId,
          ...(locals.length ? { locals } : {}),
        },
      );
      if (
        accountRef.current === accountId &&
        stateRef.current.draft?.id === workspaceId
      ) {
        for (const result of value.results)
          localVariables?.apply(
            draft,
            collectionId,
            environmentId,
            result.response?.variable_updates || [],
          );
      }
      setResult({ accountId, workspaceId, collectionId, value });
      void client.invalidateQueries({ queryKey: ["history", workspaceId] });
    } catch (error) {
      toast.error(liveError(error));
    } finally {
      running.current = false;
      setRunnerBusy(false);
    }
  }
  return {
    runCollection,
    setRunCollection,
    runnerBusy,
    runResult:
      result?.accountId === accountId &&
      result?.workspaceId === draft?.id &&
      result?.collectionId === runCollection
        ? result.value
        : null,
    run,
  };
}
