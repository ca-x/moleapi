import { liveError } from "./../../shared/i18n/errors";
import { errorCopy } from "../../shared/i18n/errors";
import type { ErrorCopy } from "../../shared/i18n/errors";
import type { LocalizedCopy } from "../../shared/i18n";
import { t, useLanguage, message, liveTranslation, translateCopy } from "../../shared/i18n";
import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { api, native, token } from "../../shared/api";
import { id, initialData } from "../../shared/model";
import type { SharedWorkspaceSummary, Workspace } from "../../shared/types";
import { hasChanges } from "../workspaces/draft";
import { useAuth } from "../auth/useAuth";
import { useWorkspace } from "../workspaces/useWorkspace";
import { resolveWorkspaceChoice } from "../workspaces/workspaceSelection";
import { fetchSharedWorkspaces } from "../workspaces/sharedWorkspacesApi";
import { useProtocolSession } from "../protocols/useProtocolSession";
import { useRequests } from "../requests/useRequests";
import { useRunner } from "../testing/useRunner";
import { useLocalVariables } from "../variables/useLocalVariables";
import { useHistory } from "../history/useHistory";
import { useSync } from "../sync/useSync";
import { useInterchange } from "../interchange/useInterchange";
import { useAppearance } from "../settings/useAppearance";
import { useInputModality } from "./useInputModality";
import type { View } from "./navigation";
export type Modal =
  | "new-workspace"
  | "new-collection"
  | "import"
  | "export"
  | "connect"
  | "search"
  | "versions"
  | "settings"
  | null;
export type Guard = {
  title: string | LocalizedCopy;
  description: string | LocalizedCopy;
  action: () => void | Promise<void>;
  saveFirst?: boolean;
};
export function useWorkbenchController() {
  useLanguage();
  useInputModality();
  const auth = useAuth();
  const queryClient = useQueryClient();
  const appearance = useAppearance();
  const workspace = useWorkspace(auth.authenticated, auth.accountId);
  const credential = token();
  const sharedBoundary = useRef({ accountId: auth.accountId, credential, epoch: 0 });
  if (sharedBoundary.current.accountId !== auth.accountId || sharedBoundary.current.credential !== credential) {
    sharedBoundary.current = { accountId: auth.accountId, credential, epoch: sharedBoundary.current.epoch + 1 };
  }
  const sharedWorkspaces = useQuery({
    queryKey: ["shared-workspaces", auth.accountId, sharedBoundary.current.epoch],
    queryFn: fetchSharedWorkspaces,
    enabled: auth.authenticated && !native,
    retry: false,
    refetchInterval: 5000,
  });
  const [sharedSelection, setSharedSelection] = useState<{ accountId: string; epoch: number; item: SharedWorkspaceSummary } | null>(null);
  const selectedShared = sharedSelection?.accountId === auth.accountId && sharedSelection.epoch === sharedBoundary.current.epoch && auth.authenticated ? sharedSelection.item : null;
  const setSelectedShared = (item: SharedWorkspaceSummary | null) => setSharedSelection(item ? { accountId: auth.accountId, epoch: sharedBoundary.current.epoch, item } : null);
  const [sharedDirty, setSharedDirty] = useState(false);
  const [view, setView] = useState<View>("requests");
  const [filter, setFilter] = useState("");
  const [sidebar, setSidebar] = useState(false);
  const [guard, setGuard] = useState<Guard | null>(null);
  const [modal, setModal] = useState<Modal>(null);
  const [versionRequestId, setVersionRequestId] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [modalError, setModalError] = useState<ErrorCopy>("");
  const [modalBusy, setModalBusy] = useState(false);
  useEffect(() => {
    setSelectedShared(null);
    setSharedDirty(false);
    setModal(null);
    setVersionRequestId(null);
    setGuard(null);
    setFilter("");
  }, [auth.accountId]);
  useEffect(() => { if (!auth.authenticated) setSelectedShared(null); }, [auth.authenticated]);
  useEffect(() => {
    if (!selectedShared || !sharedWorkspaces.data) return;
    const current = sharedWorkspaces.data.find((item) => item.owner_id === selectedShared.owner_id && item.workspace_id === selectedShared.workspace_id);
    if (!current) setSelectedShared(null);
    else if (current.role !== selectedShared.role || current.revision !== selectedShared.revision) setSelectedShared(current);
  }, [selectedShared, sharedWorkspaces.data]);
  const localVariables = useLocalVariables(workspace);
  const requests = useRequests(workspace, setView, setSidebar, localVariables);
  const protocolSession = useProtocolSession(
    workspace,
    requests.request,
    localVariables,
    auth.authenticated,
  );
  const graphqlRun = useRef<(() => void) | null>(null);
  const mcpRun = useRef<(() => void) | null>(null);
  const dataRun = useRef<(() => void) | null>(null);
  const a2aRun = useRef<(() => void) | null>(null);
  const sendRequest =
    requests.request?.protocol?.kind === "data" ? () => dataRun.current?.() : requests.request?.protocol?.kind === "a2a" ? () => a2aRun.current?.() : requests.request?.protocol?.kind === "mcp" ? () => mcpRun.current?.() : requests.request?.protocol?.kind === "graphql"
      ? () => graphqlRun.current?.()
      : requests.request?.protocol?.kind &&
          ["data", "tcp", "sse", "websocket", "grpc", "socketio", "mqtt", "mcp"].includes(
            requests.request.protocol.kind,
          )
        ? protocolSession.connect
        : requests.send;
  const runner = useRunner(workspace, localVariables);
  const history = useHistory(
    auth.authenticated,
    workspace.selectedId,
    view === "history",
    auth.accountId,
  );
  const interchange = useInterchange(workspace);
  const sync = useSync(auth.authenticated, workspace, () =>
    openModal("connect"),
  );
  function openModal(value: Modal) {
    const action = () => {
      if (selectedShared && (value === "new-workspace" || value === "import")) leaveShared();
      setModal(value);
      setModalError("");
      setName(value === "new-workspace" ? t("我的工作区") : "");
      interchange.setContent("");
      interchange.setIncludeSecrets(false);
      if (value === "export") interchange.setFormat("moleapi");
      if (value === "import") interchange.setFormat("openapi");
    };
    if (selectedShared && sharedDirty && (value === "new-workspace" || value === "import"))
      setGuard({ title: message("离开共享工作区"), description: message("共享工作区有未保存的修改。离开会放弃这些修改。"), action, saveFirst: false });
    else if (workspace.dirty && (value === "new-workspace" || value === "import"))
      setGuard({
        title: message("保留当前工作区修改"),
        description:
          message("新建或导入会打开另一个工作区。请保存当前修改，或放弃修改后继续。"),
        action,
        saveFirst: true,
      });
    else action();
  }
  function switchWorkspace(value: string) {
    const clearSharedCache = () => {
      if (!selectedShared) return;
      for (const kind of ["shared-workspace", "shared-history", "shared-reports", "shared-mock-calls"]) {
        queryClient.removeQueries({ queryKey: [kind, auth.accountId, selectedShared.owner_id, selectedShared.workspace_id] });
      }
    };
    const choice = resolveWorkspaceChoice(value, workspace.workspaces.data ?? [], sharedWorkspaces.data ?? []);
    if (!choice) { toast.error(liveTranslation("工作区尚未加载，请稍后重试。")); return; }
    if (choice.kind === "shared") {
      const shared = choice.workspace;
      if (selectedShared?.owner_id === shared.owner_id && selectedShared.workspace_id === shared.workspace_id) return;
      const action = () => { clearSharedCache(); setSharedDirty(false); workspace.setDraft(workspace.stateRef.current.saved); workspace.setSaveConflict(null); setSelectedShared(shared); setModal(null); setVersionRequestId(null); };
      if (selectedShared && sharedDirty) setGuard({ title: message("离开共享工作区"), description: message("共享工作区有未保存的修改。离开会放弃这些修改。"), action, saveFirst: false });
      else if (workspace.dirty && !selectedShared) setGuard({ title: message("切换工作区"), description: message("当前工作区有未保存修改。可以保存后切换，或放弃修改。"), action, saveFirst: true });
      else action();
      return;
    }
    const ownedId = choice.kind === "owned" ? choice.workspace.id : "";
    if (ownedId === workspace.selectedId && !selectedShared) return;
    const action = () => {
      clearSharedCache();
      setSharedDirty(false);
      setSelectedShared(null);
      workspace.setDraft(null);
      workspace.setSaved(null);
      workspace.setSelectedId(ownedId);
      setVersionRequestId(null);
      setView("requests");
    };
    if (selectedShared && sharedDirty)
      setGuard({ title: message("离开共享工作区"), description: message("共享工作区有未保存的修改。离开会放弃这些修改。"), action, saveFirst: false });
    else if (workspace.dirty)
      setGuard({
        title: message("切换工作区"),
        description: message("当前工作区有未保存修改。可以保存后切换，或放弃修改。"),
        action,
        saveFirst: true,
      });
    else action();
  }
  function leaveShared() {
    if (selectedShared) {
      for (const kind of ["shared-workspace", "shared-history", "shared-reports", "shared-mock-calls"]) {
        queryClient.removeQueries({ queryKey: [kind, auth.accountId, selectedShared.owner_id, selectedShared.workspace_id] });
      }
    }
    setSharedDirty(false);
    setSelectedShared(null);
  }
  function closeShared() {
    if (sharedDirty) setGuard({ title: message("离开共享工作区"), description: message("共享工作区有未保存的修改。离开会放弃这些修改。"), action: leaveShared, saveFirst: false });
    else leaveShared();
  }
  async function submitModal(event: React.FormEvent) {
    event.preventDefault();
    if (modalBusy) return;
    setModalBusy(true);
    setModalError("");
    try {
      if (modal === "new-workspace")
        {
          workspace.installWorkspace(await api<Workspace>("/api/workspaces", "POST", {
            name,
            data: initialData(),
          }));
          setSelectedShared(null);
        }
      else if (modal === "new-collection")
        workspace.updateData((data) => ({
          ...data,
          collections: [
            ...data.collections,
            { id: id(), name, description: "", requests: [] },
          ],
        }));
      else if (modal === "import") await interchange.importWorkspace();
      else if (modal === "export" && !(await interchange.exportWorkspace()))
        return;
      else if (modal === "connect") await sync.connectServer();
      setModal(null);
    } catch (error) {
      setModalError(errorCopy(error));
    } finally {
      setModalBusy(false);
    }
  }
  function logout() {
    const action = async () => {
      await auth.endSession();
      setSelectedShared(null);
      setSharedDirty(false);
      workspace.setDraft(null);
      workspace.setSaved(null);
      workspace.setSelectedId("");
      setModal(null);
    };
    if (selectedShared && sharedDirty)
      setGuard({ title: message("离开共享工作区"), description: message("共享工作区有未保存的修改。离开会放弃这些修改。"), action, saveFirst: false });
    else if (workspace.dirty)
      setGuard({
        title: message("退出账户"),
        description: message("工作区有未保存的修改。"),
        action,
        saveFirst: true,
      });
    else void action();
  }
  async function confirmGuard(saveFirst: boolean) {
    if (!guard) return;
    const current = guard;
    try {
      if (saveFirst) {
        if (!(await workspace.save(true))) return;
        if (hasChanges(workspace.stateRef.current)) {
          toast.message(liveTranslation("保存期间有新修改，请再次保存后继续。"));
          return;
        }
      }
      await current.action();
      setGuard(null);
    } catch (error) {
      toast.error(liveError(error));
    }
  }
  useEffect(() => {
    const listener = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey)) return;
      if (selectedShared) return;
      if (event.key.toLowerCase() === "k") {
        event.preventDefault();
        setModal("search");
        interchange.setContent("");
      } else if (
        !modal &&
        !guard &&
        !event.defaultPrevented &&
        event.key === "Enter"
      ) {
        event.preventDefault();
        void sendRequest();
      } else if (event.key.toLowerCase() === "s") {
        event.preventDefault();
        void workspace.save();
      }
    };
    window.addEventListener("keydown", listener);
    return () => window.removeEventListener("keydown", listener);
  }, [sendRequest, workspace.save, modal, guard, interchange.setContent, selectedShared]);
  const environment = workspace.draft?.data.environments.find(
    (e) => e.id === workspace.draft?.data.active_environment_id,
  );
  return {
    localVariables,
    ...auth,
    ...appearance,
    ...workspace,
    ...requests,
    protocolSession,
    graphqlRun,
    mcpRun,
    a2aRun,
    dataRun,
    send: sendRequest,
    busy: requests.busy || protocolSession.busy,
    sending: requests.sending || protocolSession.busy,
    ...runner,
    ...history,
    ...interchange,
    ...sync,
    view,
    setView,
    filter,
    setFilter,
    sidebar,
    setSidebar,
    guard,
    setGuard,
    modal,
    setModal,
    versionRequestId,
    setVersionRequestId,
    name,
    setName,
    modalError: translateCopy(modalError),
    setModalError,
    modalBusy,
    openModal,
    switchWorkspace,
    closeShared,
    selectedShared,
    setSharedDirty,
    sharedWorkspaces,
    submitModal,
    logout,
    confirmGuard,
    environment,
  };
}
