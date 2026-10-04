import { t, useLanguage } from "../../shared/i18n";
import { Flex, Text } from "@radix-ui/themes";
import { native } from "../../shared/api";
import { useWorkbench } from "../workbench/context";

export default function StatusBar() {
  useLanguage();
  const state = useWorkbench();
  const { draft, dirty } = state;
  return (
    <footer className="status-bar">
      <Flex gap="3" align="center">
        <span className="connection-state">
          {native ? t("本地工作区") : t("服务器已连接")}
        </span>
        <Text size="1" color="gray">
          {dirty ? t("有未保存修改") : draft ? t("已保存") : t("准备就绪")}
        </Text>
      </Flex>
      <Flex gap="3">
        <Text size="1" color="gray">
          {draft ? t("修订 {{value0}}", { value0: draft.revision }) : ""}
        </Text>
        <Text size="1" color="gray">
          MoleAPI 0.1.0
        </Text>
      </Flex>
    </footer>
  );
}
