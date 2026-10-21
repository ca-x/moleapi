import NativeInvitationForm from "../workspaces/NativeInvitationForm";
import { useQueryClient } from "@tanstack/react-query";
import CiPanel from "../ci/CiPanel";
import HostedRunnerManagement from "../runners/HostedRunnerManagement";
import AccessTokensPanel from "./AccessTokensPanel";
import WorkspaceMembersPanel from "./WorkspaceMembersPanel";
import SharedWorkspacesPanel from "./SharedWorkspacesPanel";
import RequestAuthEditor from "../authentication/RequestAuthEditor";
import LanguageSelector from "../../shared/i18n/LanguageSelector";
import { t, useLanguage, message } from "../../shared/i18n";
import { Button, Card, Flex, Text, TextField } from "@radix-ui/themes";
import { Choice, Field } from "../../shared/ui";
import { api, native } from "../../shared/api";
import { useWorkbench } from "../workbench/context";
export default function SettingsPanel() {
  useLanguage();
  const queryClient = useQueryClient();
  const {
    accountId,
    sharedAccountId,
    selectedShared,
    draft,
    setDraft,
    sync,
    disconnectServer,
    setModal,
    setGuard,
    setSaved,
    setSelectedId,
    switchWorkspace,
    workspaces,
    logout,
    dirty,
    save,
    dark,
    updateData,
  } = useWorkbench();
  const memberAccount = sharedAccountId ?? accountId;
  const nativeMembership = native && sync.data?.connected && sync.data.session_id && sync.data.server_url ? <NativeInvitationForm key={sync.data.session_id} scope={sync.data.session_id} serverUrl={sync.data.server_url} username={sync.data.username ?? ""} onJoined={() => void queryClient.invalidateQueries({ queryKey: ["shared-workspaces", memberAccount] })} /> : null;
  if (selectedShared) return <Flex direction="column" gap="4">
    <LanguageSelector />
    {nativeMembership}
    <SharedWorkspacesPanel key={memberAccount} accountId={memberAccount} onOpen={switchWorkspace} />
    {selectedShared.role === "admin" && <WorkspaceMembersPanel key={`${memberAccount}:${selectedShared.owner_id}:${selectedShared.workspace_id}`} accountId={memberAccount} workspaceId={selectedShared.workspace_id} ownerId={selectedShared.owner_id} invitationOrigin={native ? sync.data?.server_url : undefined} />}
    {!native && <AccessTokensPanel key={accountId} accountId={accountId} />}
    {native ? <Button color="gray" variant="soft" onClick={() => void disconnectServer()}>{t("断开自托管服务")}</Button> : <Button color="gray" variant="soft" onClick={logout}>{t("退出登录")}</Button>}
    <Button onClick={() => setModal(null)}>{t("完成")}</Button>
  </Flex>;
  return (
    <Flex direction="column" gap="4">
      <LanguageSelector />
      <Text size="2">
        {native ? t("桌面端将数据存储在本机 SQLite。") : t("当前连接自托管服务端。")}
      </Text>
      {draft && (
        <Field label={t("工作区名称")}>
          <TextField.Root
            value={draft.name}
            onChange={(e) =>
              setDraft((current) =>
                current ? { ...current, name: e.target.value } : current,
              )
            }
          />
        </Field>
      )}
      {draft && <Field label={t("工作区鉴权")}><RequestAuthEditor auth={draft.data.auth ?? {kind:"none",token:"",username:"",password:""}} change={auth=>updateData(data=>({...data,auth}))} dark={dark} inherit={false} collectionId={null}/></Field>}
      {draft && (
        <Field
          label={t("脚本兼容模式")}
          hint={t("v2 支持顶层 await；切换后，此工作区的全部前置和后置脚本都使用所选模式。")}
        >
          <Choice<"v1" | "v2">
            label={t("脚本兼容模式")}
            value={draft.data.script_mode ?? "v1"}
            onChange={(script_mode) => updateData((data) => ({ ...data, script_mode }))}
            options={[
              { value: "v1", label: t("v1（同步脚本）") },
              { value: "v2", label: t("v2（异步脚本）") },
            ]}
          />
        </Field>
      )}
      {native && sync.data?.connected && (
        <Card>
          <Text as="p" size="2">
            {sync.data.server_url}
          </Text>
          <Text as="p" size="1" color="gray">
            {sync.data.username}
          </Text>
          <Button
            color="gray"
            variant="soft"
            onClick={() => void disconnectServer()}
          > {t("断开同步")} </Button>
        </Card>
      )}
      {native && sync.data?.connected && draft && <HostedRunnerManagement key={`${sync.data.session_id}:${draft.id}`} connectionId={sync.data.session_id ?? "legacy"} workspaceId={draft.id} />}
      {draft && (
        <Button
          color="red"
          variant="soft"
          onClick={() => {
            setModal(null);
            setGuard({
              title: message("删除工作区"),
              description: message("删除此工作区及其资源与执行记录。"),
              action: async () => {
                await api(`/api/workspaces/${draft.id}`, "DELETE", {
                  expected_revision: draft.revision,
                });
                setDraft(null);
                setSaved(null);
                setSelectedId("");
                await workspaces.refetch();
              },
            });
          }}
        > {t("删除当前工作区")} </Button>
      )}
      {draft && <CiPanel/>}
      {nativeMembership}
      {(!native || sync.data?.connected) && <SharedWorkspacesPanel key={memberAccount} accountId={memberAccount} onOpen={switchWorkspace} />}
      {!native && draft && <WorkspaceMembersPanel key={`${accountId}:${draft.id}`} accountId={accountId} workspaceId={draft.id} />}
      {!native && <AccessTokensPanel key={accountId} accountId={accountId}/>}
      {!native && (
        <Button color="gray" variant="soft" onClick={logout}> {t("退出登录")} </Button>
      )}
      <Button
        onClick={async () => {
          if (dirty && !(await save())) return;
          setModal(null);
        }}
      > {t("完成")} </Button>
    </Flex>
  );
}
