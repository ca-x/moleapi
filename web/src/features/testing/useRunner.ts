import { useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { api } from "../../shared/api";
import { safeMessage } from "../../shared/model";
import type { RunResult } from "../../shared/types";
import type { useWorkspace } from "../workspaces/useWorkspace";

export function useRunner(workspace: ReturnType<typeof useWorkspace>) {
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
      const value = await api<RunResult>(
        `/api/workspaces/${workspaceId}/run`,
        "POST",
        { collection_id: collectionId, environment_id: environmentId },
      );
      setResult({ accountId, workspaceId, collectionId, value });
      void client.invalidateQueries({ queryKey: ["history", workspaceId] });
    } catch (error) {
      toast.error(safeMessage(error));
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
