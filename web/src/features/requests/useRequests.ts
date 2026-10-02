import { useCallback, useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { api } from "../../shared/api";
import { newRequest, safeMessage } from "../../shared/model";
import type { ApiResponse, RequestSpec } from "../../shared/types";
import type { useWorkspace } from "../workspaces/useWorkspace";
import type { View } from "../workbench/navigation";

type Execution = {
  accountId: string;
  workspaceId: string;
  requestId: string;
  response: ApiResponse | null;
  error: string;
};
export function useRequests(
  workspace: ReturnType<typeof useWorkspace>,
  setView: (view: View) => void,
  setSidebar: (open: boolean) => void,
) {
  const client = useQueryClient();
  const { draft, dirty, save, updateData, stateRef, accountId, accountRef } =
    workspace;
  const [requestId, setRequestId] = useState("");
  const [busy, setBusy] = useState(false);
  const running = useRef(false);
  const [execution, setExecution] = useState<Execution | null>(null);
  useEffect(() => {
    setRequestId(
      draft?.data.collections.flatMap((c) => c.requests)[0]?.id || "",
    );
    setExecution(null);
  }, [draft?.id, accountId]);
  const request = draft?.data.collections
    .flatMap((c) => c.requests)
    .find((r) => r.id === requestId);
  const updateRequest = (patch: Partial<RequestSpec>) =>
    updateData((data) => ({
      ...data,
      collections: data.collections.map((c) => ({
        ...c,
        requests: c.requests.map((r) =>
          r.id === requestId ? { ...r, ...patch } : r,
        ),
      })),
    }));
  const addRequest = (collectionId: string) => {
    const next = newRequest();
    updateData((data) => ({
      ...data,
      collections: data.collections.map((c) =>
        c.id === collectionId ? { ...c, requests: [...c.requests, next] } : c,
      ),
    }));
    setRequestId(next.id);
    setView("requests");
    setSidebar(false);
  };
  const send = useCallback(async () => {
    if (!request || !draft || running.current) return;
    const snapshot = structuredClone(request);
    const workspaceId = draft.id;
    const environmentId = draft.data.active_environment_id;
    const requestId = snapshot.id;
    running.current = true;
    setBusy(true);
    setExecution({
      accountId,
      workspaceId,
      requestId,
      response: null,
      error: "",
    });
    try {
      if (dirty && !(await save(true))) return;
      if (
        accountRef.current !== accountId ||
        stateRef.current.draft?.id !== workspaceId
      )
        return;
      const response = await api<ApiResponse>("/api/execute", "POST", {
        workspace_id: workspaceId,
        request: snapshot,
        environment_id: environmentId,
      });
      setExecution({ accountId, workspaceId, requestId, response, error: "" });
      void client.invalidateQueries({ queryKey: ["history", workspaceId] });
    } catch (error) {
      setExecution({
        accountId,
        workspaceId,
        requestId,
        response: null,
        error: safeMessage(error),
      });
    } finally {
      running.current = false;
      setBusy(false);
    }
  }, [request, draft, dirty, save, client, stateRef, accountId, accountRef]);
  const current =
    execution?.accountId === accountId &&
    execution?.workspaceId === draft?.id &&
    execution?.requestId === requestId
      ? execution
      : null;
  return {
    requestId,
    setRequestId,
    request,
    updateRequest,
    addRequest,
    send,
    busy: busy && !!current,
    sending: busy,
    response: current?.response || null,
    requestError: current?.error || "",
    setResponse: () => setExecution(null),
    setRequestError: () => setExecution(null),
  };
}
