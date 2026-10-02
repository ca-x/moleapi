import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import { api, native } from "../../shared/api";
import { fingerprint, safeMessage } from "../../shared/model";
import type { SyncResult, SyncStatus } from "../../shared/types";
import { hasChanges } from "../workspaces/draft";
import type { useWorkspace } from "../workspaces/useWorkspace";
export function useSync(
  authenticated: boolean,
  workspace: ReturnType<typeof useWorkspace>,
  connect: () => void,
) {
  const sync = useQuery({
    queryKey: ["sync-status"],
    queryFn: () => api<SyncStatus>("/api/sync/status"),
    enabled: authenticated && native,
  });
  const [serverUrl, setServerUrl] = useState("");
  const [serverUser, setServerUser] = useState("");
  const [serverPassword, setServerPassword] = useState("");
  const [syncBusy, setSyncBusy] = useState(false);
  const [syncConflict, setSyncConflict] = useState<SyncResult | null>(null);
  async function connectServer() {
    await api("/api/sync/connect", "POST", {
      server_url: serverUrl,
      username: serverUser,
      password: serverPassword,
    });
    setServerPassword("");
    await sync.refetch();
    toast.success("已连接自托管服务");
  }
  async function disconnectServer() {
    try {
      await api("/api/sync/connect", "DELETE");
      await sync.refetch();
      toast.success("已断开同步，本地数据保留");
    } catch (error) {
      toast.error(safeMessage(error));
    }
  }
  async function synchronize(resolution?: "push" | "pull") {
    const { draft, stateRef, dirty, save, installWorkspace } = workspace;
    if (!draft || syncBusy) return;
    if (!sync.data?.connected) {
      connect();
      return;
    }
    setSyncBusy(true);
    try {
      if (dirty && !(await save(true))) return;
      if (hasChanges(stateRef.current)) {
        toast.message("保存期间有新修改，请保存后再次同步。");
        return;
      }
      const snapshot = stateRef.current.draft;
      if (!snapshot || snapshot.id !== draft.id) return;
      const result = await api<SyncResult>(
        `/api/workspaces/${snapshot.id}/sync`,
        "POST",
        { resolution },
      );
      const latest = stateRef.current.draft;
      if (
        latest?.id !== snapshot.id ||
        fingerprint(latest) !== fingerprint(snapshot)
      ) {
        if (latest?.id === snapshot.id)
          workspace.setSaveConflict(result.workspace);
        toast.message("同步期间有新修改，当前修改已保留。请确认版本后继续。");
        return;
      }
      if (result.status === "conflict") setSyncConflict(result);
      else {
        installWorkspace(result.workspace);
        setSyncConflict(null);
        toast.success(result.message || "同步完成");
      }
    } catch (error) {
      toast.error(safeMessage(error));
    } finally {
      setSyncBusy(false);
    }
  }
  return {
    sync,
    serverUrl,
    setServerUrl,
    serverUser,
    setServerUser,
    serverPassword,
    setServerPassword,
    syncBusy,
    syncConflict,
    setSyncConflict,
    connectServer,
    disconnectServer,
    synchronize,
  };
}
