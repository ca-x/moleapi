import { t, useLanguage } from "./shared/i18n";
import { Button, Callout, Heading, Text, Theme } from "@radix-ui/themes";
import { Toaster } from "sonner";
import AuthScreen from "./features/auth/AuthScreen";
import WorkbenchHeader from "./features/workbench/WorkbenchHeader";
import NavigationRail from "./features/workbench/NavigationRail";
import CollectionSidebar from "./features/requests/CollectionSidebar";
import WorkbenchMain from "./features/workbench/WorkbenchMain";
import StatusBar from "./features/workbench/StatusBar";
import WorkbenchDialogs from "./features/workbench/WorkbenchDialogs";
import GuardDialog from "./features/workspaces/GuardDialog";
import SaveConflictDialog from "./features/workspaces/SaveConflictDialog";
import RequestVersionDialog from "./features/requests/RequestVersionDialog";
import SyncConflictDialog from "./features/sync/SyncConflictDialog";
import WorkspaceInvitationPrompt from "./features/workspaces/WorkspaceInvitationPrompt";
import SharedWorkspaceView from "./features/workspaces/SharedWorkspaceView";
import { WorkbenchContext } from "./features/workbench/context";
import { useWorkbenchController } from "./features/workbench/useWorkbenchController";
import { safeMessage } from "./shared/model";
export default function App() {
  useLanguage();
  const state = useWorkbenchController();
  const { dark, status, authenticated, login } = state;
  return (
    <Theme
      appearance={dark ? "dark" : "light"}
      accentColor="cyan"
      grayColor="slate"
      radius="medium"
    >
      <Toaster
        theme={dark ? "dark" : "light"}
        position="bottom-right"
        closeButton
        richColors
      />
      {status.isPending ? (
        <main className="boot-screen" role="status">
          <img src="/logo.png" alt="MoleAPI" width="64" height="64" />
          <Text>{t("正在打开工作台…")}</Text>
        </main>
      ) : status.error ? (
        <main className="boot-screen">
          <Heading size="5">{t("无法连接工作台")}</Heading>
          <Callout.Root color="red">
            <Callout.Text>{safeMessage(status.error)}</Callout.Text>
          </Callout.Root>
          <Button onClick={() => status.refetch()}>{t("重试")}</Button>
        </main>
      ) : !authenticated && status.data ? (
        <AuthScreen status={status.data} onLogin={login} />
      ) : (
        <WorkbenchContext.Provider value={state}>
          <div className={`app-shell${state.selectedShared ? " shared-shell" : ""}`}>
            <WorkbenchHeader />
            {state.selectedShared ? <SharedWorkspaceView key={`${state.sharedAccountId}:${state.selectedShared.owner_id}:${state.selectedShared.workspace_id}:${state.selectedShared.role}`} accountId={state.sharedAccountId} selected={state.selectedShared} dark={dark} onClose={state.closeShared} onDirtyChange={state.setSharedDirty} /> : <><NavigationRail /><CollectionSidebar /><WorkbenchMain /><StatusBar /></>}
          </div>
          <WorkbenchDialogs />
          <GuardDialog />
          <SaveConflictDialog />
          <RequestVersionDialog />
          <SyncConflictDialog />
          <WorkspaceInvitationPrompt key={state.accountId} accountId={state.accountId} />
        </WorkbenchContext.Provider>
      )}
    </Theme>
  );
}
