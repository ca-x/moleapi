import { Button, Dialog, Flex, Text } from "@radix-ui/themes";
import { saveFile } from "../../shared/api";
import { safeMessage } from "../../shared/model";
import { toast } from "sonner";
import { useWorkbench } from "../workbench/context";
export default function SaveConflictDialog() {
  const {
    saveConflict,
    setSaveConflict,
    keepConflictEdits,
    useConflictRemote,
    draft,
  } = useWorkbench();
  return (
    <Dialog.Root
      open={!!saveConflict}
      onOpenChange={(open) => {
        if (!open) setSaveConflict(null);
      }}
    >
      <Dialog.Content maxWidth="560px">
        <Dialog.Title>工作区已被更新</Dialog.Title>
        <Dialog.Description>
          服务器版本已改变。当前未保存修改完整保留，请选择如何继续。
        </Dialog.Description>
        <Text as="p" size="2" my="4">
          服务器修订 {saveConflict?.revision}
          。保留修改后，再次点击保存将使用此修订提交你的完整工作区。
        </Text>
        <Flex gap="3" wrap="wrap">
          <Button
            variant="soft"
            color="gray"
            onClick={async () => {
              if (!draft) return;
              try {
                await saveFile({
                  filename: `moleapi-unsaved-${draft.id}.json`,
                  content: JSON.stringify(
                    { name: draft.name, data: draft.data },
                    null,
                    2,
                  ),
                  mime: "application/json",
                });
              } catch (error) {
                toast.error(safeMessage(error));
              }
            }}
          >
            备份当前修改（包含密钥）
          </Button>
          <Button variant="outline" onClick={useConflictRemote}>
            放弃修改，使用服务器版本
          </Button>
          <Button onClick={keepConflictEdits}>保留修改，准备重试</Button>
        </Flex>
      </Dialog.Content>
    </Dialog.Root>
  );
}
