import { AlertDialog, Button, Flex } from "@radix-ui/themes";
import { useWorkbench } from "../workbench/context";

export default function GuardDialog() {
  const state = useWorkbench();
  const { saving, guard, setGuard, confirmGuard } = state;
  return (
    <AlertDialog.Root
      open={!!guard}
      onOpenChange={(open) => {
        if (!open) setGuard(null);
      }}
    >
      <AlertDialog.Content maxWidth="460px">
        <AlertDialog.Title>{guard?.title}</AlertDialog.Title>
        <AlertDialog.Description>{guard?.description}</AlertDialog.Description>
        <Flex gap="3" justify="end" mt="5">
          <AlertDialog.Cancel>
            <Button color="gray" variant="soft">
              取消
            </Button>
          </AlertDialog.Cancel>
          <Button
            color={guard?.saveFirst ? "gray" : "red"}
            variant={guard?.saveFirst ? "outline" : "solid"}
            onClick={() => void confirmGuard(false)}
          >
            {guard?.saveFirst ? "放弃修改" : "确认"}
          </Button>
          {guard?.saveFirst && (
            <Button loading={saving} onClick={() => void confirmGuard(true)}>
              保存后继续
            </Button>
          )}
        </Flex>
      </AlertDialog.Content>
    </AlertDialog.Root>
  );
}
