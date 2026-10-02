import { Button, Card, Flex, Text, TextField } from "@radix-ui/themes";
import { Field } from "../../shared/ui";
import { api, native } from "../../shared/api";
import { useWorkbench } from "../workbench/context";
export default function SettingsPanel() {
  const {
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
  } = useWorkbench();
  return (
    <Flex direction="column" gap="4">
      <Text size="2">
        {native ? "桌面端将数据存储在本机 SQLite。" : "当前连接自托管服务端。"}
      </Text>
      {draft && (
        <Field label="工作区名称">
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
          >
            断开同步
          </Button>
        </Card>
      )}
      {draft && (
        <Button
          color="red"
          variant="soft"
          onClick={() => {
            setModal(null);
            setGuard({
              title: "删除工作区",
              description: "删除此工作区及其资源与执行记录。",
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
        >
          删除当前工作区
        </Button>
      )}
      {!native && (
        <Button color="gray" variant="soft" onClick={logout}>
          退出登录
        </Button>
      )}
      <Button
        onClick={async () => {
          if (dirty && !(await save())) return;
          setModal(null);
        }}
      >
        完成
      </Button>
    </Flex>
  );
}
