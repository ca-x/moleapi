import { t, useLanguage } from "../../shared/i18n";
import { Button, Callout, Dialog, Flex, TextField } from "@radix-ui/themes";
import { Field } from "../../shared/ui";
import RequestSearch from "../requests/RequestSearch";
import SettingsPanel from "../settings/SettingsPanel";
import InterchangeFields from "../interchange/InterchangeFields";
import ConnectionFields from "../sync/ConnectionFields";
import { useWorkbench } from "./context";
const titles = {
  get "new-workspace"() { return t("新建工作区"); },
  get "new-collection"() { return t("新建集合"); },
  get import() { return t("导入 API 数据"); },
  get export() { return t("导出工作区"); },
  get connect() { return t("连接自托管服务"); },
  get search() { return t("搜索请求"); },
  get settings() { return t("工作台设置"); },
};
export default function WorkbenchDialogs() {
  useLanguage();
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
            ? t("登录你自己的服务端，按工作区选择同步。")
            : modal === "search"
              ? t("按名称、方法或 URL 查找当前工作区的请求。")
              : t("管理当前工作区的数据与设置。")}
        </Dialog.Description>
        {modal === "search" ? (
          <RequestSearch />
        ) : modal === "settings" ? (
          <SettingsPanel />
        ) : (
          <form onSubmit={submitModal}>
            <Flex direction="column" gap="4" mt="4">
              {(modal === "new-workspace" || modal === "new-collection") && (
                <Field label={t("名称")}>
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
                  > {t("取消")} </Button>
                </Dialog.Close>
                <Button type="submit" loading={modalBusy}>
                  {modal === "export"
                    ? t("导出")
                    : modal === "import"
                      ? t("导入")
                      : modal === "connect"
                        ? t("连接")
                        : t("创建")}
                </Button>
              </Flex>
            </Flex>
          </form>
        )}
      </Dialog.Content>
    </Dialog.Root>
  );
}
