import { Badge, Button, Flex, Text } from "@radix-ui/themes";
import {
  Cloud,
  CloudOff,
  Menu,
  Moon,
  Plus,
  Search,
  Settings,
  Sun,
} from "lucide-react";
import { Choice, ToolButton } from "../../shared/ui";
import { native } from "../../shared/api";
import { useWorkbench } from "../workbench/context";

export default function WorkbenchHeader() {
  const state = useWorkbench();
  const {
    dark,
    setDark,
    workspaces,
    sync,
    selectedId,
    sidebar,
    setSidebar,
    syncBusy,
    openModal,
    switchWorkspace,
    synchronize,
  } = state;
  return (
    <header className="app-header">
      <Flex align="center" gap="3">
        <ToolButton
          label={sidebar ? "收起目录" : "展开目录"}
          className="sidebar-toggle"
          expanded={sidebar}
          controls="collection-sidebar"
          onClick={() => setSidebar(!sidebar)}
        >
          <Menu size={18} />
        </ToolButton>
        <img src="/logo.png" alt="" width="36" height="36" />
        <Text weight="bold" size="3">
          MoleAPI
        </Text>
        <span className="header-divider" />
        {workspaces.data?.length ? (
          <Choice
            value={selectedId || workspaces.data[0].id}
            onChange={switchWorkspace}
            options={workspaces.data.map((w) => ({
              value: w.id,
              label: w.name,
            }))}
            label="工作区"
          />
        ) : (
          <Text size="2" color="gray">
            API 工作台
          </Text>
        )}
        <ToolButton
          label="新建工作区"
          onClick={() => openModal("new-workspace")}
        >
          <Plus size={16} />
        </ToolButton>
      </Flex>
      <Flex align="center" gap="4">
        <Button
          variant="ghost"
          color="gray"
          className="search-trigger"
          onClick={() => openModal("search")}
        >
          <Search size={16} />
          <span>搜索请求</span>
          <kbd>⌘ K</kbd>
        </Button>
        {native ? (
          <Button
            variant="soft"
            color={sync.data?.connected ? "cyan" : "gray"}
            loading={syncBusy}
            onClick={() => synchronize()}
          >
            {sync.data?.connected ? (
              <Cloud size={16} />
            ) : (
              <CloudOff size={16} />
            )}
            <span>{sync.data?.connected ? "同步" : "连接服务器"}</span>
          </Button>
        ) : (
          <Badge color="gray" variant="soft">
            自托管
          </Badge>
        )}
        <ToolButton
          label={dark ? "切换为浅色" : "切换为深色"}
          onClick={() => setDark(!dark)}
        >
          {dark ? <Sun size={17} /> : <Moon size={17} />}
        </ToolButton>
        <ToolButton label="工作台设置" onClick={() => openModal("settings")}>
          <Settings size={17} />
        </ToolButton>
      </Flex>
    </header>
  );
}
