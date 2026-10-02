import { useCallback, useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { api, ApiError } from "../../shared/api";
import { safeMessage } from "../../shared/model";
import type { Workspace, WorkspaceData } from "../../shared/types";
import { acknowledgeSave, hasChanges, retainLocal } from "./draft";
import type { DraftState } from "./draft";

export function useWorkspace(authenticated: boolean, accountId = "local") {
  const client = useQueryClient();
  const workspaces = useQuery({
    queryKey: ["workspaces", accountId],
    queryFn: () => api<Workspace[]>("/api/workspaces"),
    enabled: authenticated,
  });
  const [selectedId, setSelectedId] = useState("");
  const [state, setState] = useState<DraftState>({ draft: null, saved: null });
  const stateRef = useRef(state);
  const accountRef = useRef(accountId);
  const accountDrafts = useRef(new Map<string, DraftState>());
  const [saving, setSaving] = useState(false);
  const [saveConflict, setSaveConflict] = useState<Workspace | null>(null);
  const pendingSave = useRef<Promise<Workspace | null> | null>(null);
  const change = useCallback((update: (state: DraftState) => DraftState) => {
    stateRef.current = update(stateRef.current);
    setState(stateRef.current);
  }, []);
  const setDraft = useCallback(
    (
      update:
        Workspace | null | ((draft: Workspace | null) => Workspace | null),
    ) => {
      change((current) => ({
        ...current,
        draft: typeof update === "function" ? update(current.draft) : update,
      }));
    },
    [change],
  );
  const setSaved = useCallback(
    (saved: Workspace | null) => change((current) => ({ ...current, saved })),
    [change],
  );
  const updateData = useCallback(
    (update: (data: WorkspaceData) => WorkspaceData) => {
      setDraft((current) =>
        current ? { ...current, data: update(current.data) } : current,
      );
    },
    [setDraft],
  );
  const installWorkspace = useCallback(
    (item: Workspace) => {
      client.setQueryData<Workspace[]>(["workspaces", accountId], (old) => [
        ...(old || []).filter((w) => w.id !== item.id),
        item,
      ]);
      setSelectedId(item.id);
      change(() => ({ draft: item, saved: item }));
    },
    [client, change, accountId],
  );
  useEffect(() => {
    if (accountRef.current !== accountId) {
      accountDrafts.current.set(accountRef.current, stateRef.current);
      accountRef.current = accountId;
      const restored = accountDrafts.current.get(accountId) || {
        draft: null,
        saved: null,
      };
      change(() => restored);
      setSelectedId(restored.draft?.id || "");
      setSaveConflict(null);
      return;
    }
    if (!authenticated) return;
    const item =
      workspaces.data?.find((w) => w.id === selectedId) ||
      (!selectedId ? workspaces.data?.[0] : undefined);
    if (item && stateRef.current.draft?.id !== item.id) installWorkspace(item);
  }, [
    selectedId,
    workspaces.data,
    authenticated,
    installWorkspace,
    accountId,
    change,
  ]);
  const dirty = hasChanges(state);
  useEffect(() => {
    const listener = (event: BeforeUnloadEvent) => {
      if (hasChanges(stateRef.current)) {
        event.preventDefault();
        event.returnValue = "";
      }
    };
    window.addEventListener("beforeunload", listener);
    return () => window.removeEventListener("beforeunload", listener);
  }, []);
  const save = useCallback(
    (silent = false): Promise<Workspace | null> => {
      if (pendingSave.current) return pendingSave.current;
      const current = stateRef.current.draft;
      if (!current) return Promise.resolve(null);
      const snapshot = structuredClone(current);
      setSaving(true);
      const operation = (async () => {
        try {
          const next = await api<Workspace>(
            `/api/workspaces/${snapshot.id}`,
            "PUT",
            {
              name: snapshot.name,
              data: snapshot.data,
              expected_revision: snapshot.revision,
            },
          );
          client.setQueryData<Workspace[]>(["workspaces", accountId], (old) =>
            (old || []).map((w) => (w.id === next.id ? next : w)),
          );
          if (accountRef.current === accountId) {
            change((latest) => acknowledgeSave(latest, next));
            setSaveConflict(null);
          }
          if (!silent) toast.success("已保存");
          return next;
        } catch (error) {
          if (
            accountRef.current === accountId &&
            error instanceof ApiError &&
            error.status === 409
          ) {
            try {
              const remote = await api<Workspace>(
                `/api/workspaces/${snapshot.id}`,
              );
              if (
                accountRef.current === accountId &&
                stateRef.current.draft?.id === snapshot.id
              )
                setSaveConflict(remote);
            } catch (fetchError) {
              toast.error(safeMessage(fetchError));
            }
          }
          toast.error(safeMessage(error));
          return null;
        } finally {
          setSaving(false);
          pendingSave.current = null;
        }
      })();
      pendingSave.current = operation;
      return operation;
    },
    [client, change, accountId],
  );
  const keepConflictEdits = () => {
    if (saveConflict) change((current) => retainLocal(current, saveConflict));
    setSaveConflict(null);
  };
  const useConflictRemote = () => {
    if (saveConflict) installWorkspace(saveConflict);
    setSaveConflict(null);
  };
  return {
    accountId,
    accountRef,
    workspaces,
    selectedId: accountRef.current === accountId ? selectedId : "",
    setSelectedId,
    ...state,
    draft: accountRef.current === accountId ? state.draft : null,
    saved: accountRef.current === accountId ? state.saved : null,
    stateRef,
    setDraft,
    setSaved,
    dirty,
    saving,
    updateData,
    installWorkspace,
    save,
    saveConflict,
    setSaveConflict,
    keepConflictEdits,
    useConflictRemote,
  };
}
