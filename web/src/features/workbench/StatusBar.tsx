import { Flex, Text } from "@radix-ui/themes";
import { native } from "../../shared/api";
import { useWorkbench } from "../workbench/context";

export default function StatusBar() {
  const state = useWorkbench();
  const { draft, dirty } = state;
  return (
    <footer className="status-bar">
      <Flex gap="3" align="center">
        <span className="connection-state">
          {native ? "本地工作区" : "服务器已连接"}
        </span>
        <Text size="1" color="gray">
          {dirty ? "有未保存修改" : draft ? "已保存" : "准备就绪"}
        </Text>
      </Flex>
      <Flex gap="3">
        <Text size="1" color="gray">
          {draft ? `修订 ${draft.revision}` : ""}
        </Text>
        <Text size="1" color="gray">
          MoleAPI 0.1.0
        </Text>
      </Flex>
    </footer>
  );
}
