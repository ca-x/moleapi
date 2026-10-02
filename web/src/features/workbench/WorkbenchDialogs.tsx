import { Button, Callout, Dialog, Flex, TextField } from "@radix-ui/themes";
import { Field } from "../../shared/ui";
import RequestSearch from "../requests/RequestSearch";
import SettingsPanel from "../settings/SettingsPanel";
import InterchangeFields from "../interchange/InterchangeFields";
import ConnectionFields from "../sync/ConnectionFields";
import { useWorkbench } from "./context";
const titles = {
  "new-workspace": "新建工作区",
  "new-collection": "新建集合",
  import: "导入 API 数据",
  export: "导出工作区",
  connect: "连接自托管服务",
  search: "搜索请求",
  settings: "工作台设置",
};
export default function WorkbenchDialogs() {
  const { modal, setModal, modalBusy, modalError, submitModal, name, setName } =
    useWorkbench();
  return (
    <Dialog.Root
      open={!!modal}
      onOpenChange={(open) => {
        if (!open && !modalBusy) setModal(null);
      }}
    >
      <Dialog.Content
        maxWidth={modal === "import" || modal === "search" ? "720px" : "560px"}
        className={modal === "search" ? "command-dialog" : ""}
      >
        <Dialog.Title>{modal ? titles[modal] : ""}</Dialog.Title>
        <Dialog.Description>
          {modal === "connect"
            ? "登录你自己的服务端，按工作区选择同步。"
            : modal === "search"
              ? "按名称、方法或 URL 查找当前工作区的请求。"
              : "管理当前工作区的数据与设置。"}
        </Dialog.Description>
        {modal === "search" ? (
          <RequestSearch />
        ) : modal === "settings" ? (
          <SettingsPanel />
        ) : (
          <form onSubmit={submitModal}>
            <Flex direction="column" gap="4" mt="4">
              {(modal === "new-workspace" || modal === "new-collection") && (
                <Field label="名称">
                  <TextField.Root
                    required
                    autoFocus
                    maxLength={128}
                    value={name}
                    onChange={(e) => setName(e.target.value)}
                  />
                </Field>
              )}
              {(modal === "import" || modal === "export") && (
                <InterchangeFields />
              )}
              {modal === "connect" && <ConnectionFields />}
              {modalError && (
                <Callout.Root color="red" role="alert">
                  <Callout.Text>{modalError}</Callout.Text>
                </Callout.Root>
              )}
              <Flex gap="3" justify="end">
                <Dialog.Close>
                  <Button
                    type="button"
                    disabled={modalBusy}
                    color="gray"
                    variant="soft"
                  >
                    取消
                  </Button>
                </Dialog.Close>
                <Button type="submit" loading={modalBusy}>
                  {modal === "export"
                    ? "导出"
                    : modal === "import"
                      ? "导入"
                      : modal === "connect"
                        ? "连接"
                        : "创建"}
                </Button>
              </Flex>
            </Flex>
          </form>
        )}
      </Dialog.Content>
    </Dialog.Root>
  );
}
