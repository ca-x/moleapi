import CiPanel from "../ci/CiPanel";
import AccessTokensPanel from "./AccessTokensPanel";
import RequestAuthEditor from "../authentication/RequestAuthEditor";
import LanguageSelector from "../../shared/i18n/LanguageSelector";
import { t, useLanguage, message } from "../../shared/i18n";
import { Button, Card, Flex, Text, TextField } from "@radix-ui/themes";
import { Field } from "../../shared/ui";
import { api, native } from "../../shared/api";
import { useWorkbench } from "../workbench/context";
export default function SettingsPanel() {
  useLanguage();
  const {
    accountId,
    draft,
    setDraft,
    sync,
    disconnectServer,
    setModal,
    setGuard,
    setSaved,
    setSelectedId,
    workspaces,
    logout,
    dirty,
    save,
    dark,
    updateData,
  } = useWorkbench();
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
