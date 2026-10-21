import { liveError } from "./../../shared/i18n/errors";
import { useLanguage, liveTranslation } from "../../shared/i18n";
import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { api, native } from "../../shared/api";
import { applyNativeMemberStatus, beginNativeMemberChange, nativeMemberSession, subscribeNativeMember } from "../../shared/nativeMemberSession";
import { fingerprint } from "../../shared/model";
import type { SyncResult, SyncStatus } from "../../shared/types";
import { hasChanges } from "../workspaces/draft";
import type { useWorkspace } from "../workspaces/useWorkspace";
export function useSync(
  authenticated: boolean,
  workspace: ReturnType<typeof useWorkspace>,
  connect: () => void,
) {
  useLanguage();
  const client = useQueryClient();
  const connectionAttempt = useRef<{ generation: number; sessionId?: string } | null>(null);
  function connectionAttemptCurrent() {
    const attempt = connectionAttempt.current, current = nativeMemberSession();
    return !!attempt && (current.sessionId ? current.sessionId === attempt.sessionId : current.epoch === attempt.generation);
  }
  useEffect(() => {
    if (!native) return;
    let previous = nativeMemberSession();
    return subscribeNativeMember(() => {
      const current = nativeMemberSession();
      if (current === previous) return;
      previous = current;
      client.removeQueries({ predicate: query => String(query.queryKey[0]).startsWith("shared-") || query.queryKey[0] === "workspace-membership" });
      if (!current.sessionId) client.setQueryData<SyncStatus>(["sync-status"], { connected: false });
    });
  }, [client]);
  const sync = useQuery({
    queryKey: ["sync-status"],
    queryFn: async () => {
      const origin = nativeMemberSession();
      const status = await api<SyncStatus>("/api/sync/status");
      if (nativeMemberSession() !== origin) return client.getQueryData<SyncStatus>(["sync-status"]) ?? { connected: false };
      return applyNativeMemberStatus(status, origin.epoch) ? status : { connected: false };
    },
    enabled: authenticated && native,
  });
  const [serverUrl, setServerUrl] = useState("");
  const [serverUser, setServerUser] = useState("");
  const [serverPassword, setServerPassword] = useState("");
  const [syncBusy, setSyncBusy] = useState(false);
  const [syncConflict, setSyncConflict] = useState<SyncResult | null>(null);
  async function connectServer() {
    const attempt = { generation: nativeMemberSession().epoch + 1, sessionId: undefined as string | undefined };
    connectionAttempt.current = attempt;
    const generation = beginNativeMemberChange();
    let status: SyncStatus;
    try {
      status = await api<SyncStatus>("/api/sync/connect", "POST", { server_url: serverUrl, username: serverUser, password: serverPassword });
    } catch (error) { if (nativeMemberSession().epoch !== generation) return false; throw error; }
    attempt.sessionId = status.connected ? status.session_id : undefined;
    if (!applyNativeMemberStatus(status, generation)) return false;
    const connected = nativeMemberSession();
    client.setQueryData(["sync-status"], status);
    setServerPassword("");
    await sync.refetch();
    if (nativeMemberSession() !== connected) return false;
    toast.success(liveTranslation("已连接自托管服务"));
    return true;
  }
  async function disconnectServer() {
    connectionAttempt.current = null;
    const generation = beginNativeMemberChange();
    try {
      await api("/api/sync/connect", "DELETE");
      if (nativeMemberSession().epoch !== generation) return;
      client.setQueryData<SyncStatus>(["sync-status"], { connected: false });
      toast.success(liveTranslation("已断开同步，本地数据保留"));
    } catch (error) {
      if (nativeMemberSession().epoch === generation) toast.error(liveError(error));
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
        toast.message(liveTranslation("保存期间有新修改，请保存后再次同步。"));
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
        toast.message(liveTranslation("同步期间有新修改，当前修改已保留。请确认版本后继续。"));
        return;
      }
      if (result.status === "conflict") setSyncConflict(result);
      else {
        installWorkspace(result.workspace);
        setSyncConflict(null);
        toast.success(result.message || liveTranslation("同步完成"));
      }
    } catch (error) {
      toast.error(liveError(error));
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
    connectionAttemptCurrent,
    disconnectServer,
    synchronize,
  };
}
