import { Button, Card, Dialog, Flex, Text } from "@radix-ui/themes";
import { useWorkbench } from "../workbench/context";

export default function SyncConflictDialog() {
  const state = useWorkbench();
  const { syncBusy, syncConflict, setSyncConflict, openModal, synchronize } =
    state;
  return (
    <Dialog.Root
      open={!!syncConflict}
      onOpenChange={(open) => {
        if (!open) setSyncConflict(null);
      }}
    >
      <Dialog.Content maxWidth="560px">
        <Dialog.Title>发现同步冲突</Dialog.Title>
        <Dialog.Description>
          本地与服务器都有修改。当前数据尚未覆盖，请选择要保留的版本。
        </Dialog.Description>
        <Flex gap="3" my="5">
          <Card>
            <Text size="2">本地修订 {syncConflict?.workspace.revision}</Text>
          </Card>
          <Card>
            <Text size="2">服务器修订 {syncConflict?.remote?.revision}</Text>
          </Card>
        </Flex>
        <Flex gap="3" wrap="wrap">
          <Button
            color="gray"
            variant="soft"
            onClick={() => openModal("export")}
          >
            先导出本地副本
          </Button>
          <Button
            variant="outline"
            loading={syncBusy}
            onClick={() => synchronize("pull")}
          >
            使用服务器版本
          </Button>
          <Button loading={syncBusy} onClick={() => synchronize("push")}>
            使用本地版本
          </Button>
        </Flex>
      </Dialog.Content>
    </Dialog.Root>
  );
}
