import { useEffect, useState } from "react";
import { toast } from "sonner";
import { api } from "../../shared/api";
import { id, initialData, safeMessage } from "../../shared/model";
import type { Workspace } from "../../shared/types";
import { hasChanges } from "../workspaces/draft";
import { useAuth } from "../auth/useAuth";
import { useWorkspace } from "../workspaces/useWorkspace";
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
  | "settings"
  | null;
export type Guard = {
  title: string;
  description: string;
  action: () => void | Promise<void>;
  saveFirst?: boolean;
};
export function useWorkbenchController() {
  useInputModality();
  const auth = useAuth();
  const appearance = useAppearance();
  const workspace = useWorkspace(auth.authenticated, auth.accountId);
  const [view, setView] = useState<View>("requests");
  const [filter, setFilter] = useState("");
  const [sidebar, setSidebar] = useState(false);
  const [guard, setGuard] = useState<Guard | null>(null);
  const [modal, setModal] = useState<Modal>(null);
  const [name, setName] = useState("");
  const [modalError, setModalError] = useState("");
  const [modalBusy, setModalBusy] = useState(false);
  useEffect(() => {
    setModal(null);
    setGuard(null);
    setFilter("");
  }, [auth.accountId]);
  const localVariables = useLocalVariables(workspace);
  const requests = useRequests(workspace, setView, setSidebar, localVariables);
  const protocolSession = useProtocolSession(
    workspace,
    requests.request,
    localVariables,
    auth.authenticated,
  );
  const sendRequest =
    requests.request?.protocol?.kind &&
    requests.request.protocol.kind !== "http"
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
      setModal(value);
      setModalError("");
      setName(value === "new-workspace" ? "我的工作区" : "");
      interchange.setContent("");
      interchange.setIncludeSecrets(false);
      if (value === "export") interchange.setFormat("moleapi");
      if (value === "import") interchange.setFormat("openapi");
    };
    if (workspace.dirty && (value === "new-workspace" || value === "import"))
      setGuard({
        title: "保留当前工作区修改",
        description:
          "新建或导入会打开另一个工作区。请保存当前修改，或放弃修改后继续。",
        action,
        saveFirst: true,
      });
    else action();
  }
  function switchWorkspace(value: string) {
    if (value === workspace.selectedId) return;
    const action = () => {
      workspace.setDraft(null);
      workspace.setSaved(null);
      workspace.setSelectedId(value);
      setView("requests");
    };
    if (workspace.dirty)
      setGuard({
        title: "切换工作区",
        description: "当前工作区有未保存修改。可以保存后切换，或放弃修改。",
        action,
        saveFirst: true,
      });
    else action();
  }
  async function submitModal(event: React.FormEvent) {
    event.preventDefault();
    if (modalBusy) return;
    setModalBusy(true);
    setModalError("");
    try {
      if (modal === "new-workspace")
        workspace.installWorkspace(
          await api<Workspace>("/api/workspaces", "POST", {
            name,
            data: initialData(),
          }),
        );
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
      setModalError(safeMessage(error));
    } finally {
      setModalBusy(false);
    }
  }
  function logout() {
    const action = async () => {
      await auth.endSession();
      workspace.setDraft(null);
      workspace.setSaved(null);
      workspace.setSelectedId("");
      setModal(null);
    };
    if (workspace.dirty)
      setGuard({
        title: "退出账户",
        description: "工作区有未保存的修改。",
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
          toast.message("保存期间有新修改，请再次保存后继续。");
          return;
        }
      }
      await current.action();
      setGuard(null);
    } catch (error) {
      toast.error(safeMessage(error));
    }
  }
  useEffect(() => {
    const listener = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey)) return;
      if (event.key.toLowerCase() === "k") {
        event.preventDefault();
        setModal("search");
        interchange.setContent("");
      } else if (!modal && !guard && event.key === "Enter") {
        event.preventDefault();
        void sendRequest();
      } else if (event.key.toLowerCase() === "s") {
        event.preventDefault();
        void workspace.save();
      }
    };
    window.addEventListener("keydown", listener);
    return () => window.removeEventListener("keydown", listener);
  }, [sendRequest, workspace.save, modal, guard, interchange.setContent]);
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
    name,
    setName,
    modalError,
    setModalError,
    modalBusy,
    openModal,
    switchWorkspace,
    submitModal,
    logout,
    confirmGuard,
    environment,
  };
}
