import { t, useLanguage } from "../../shared/i18n";
import { Badge, Button, Flex, Text, Heading } from "@radix-ui/themes";
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
import { ownedWorkspaceKey, sharedWorkspaceKey } from "../workspaces/workspaceSelection";

export default function WorkbenchHeader() {
  useLanguage();
  const state = useWorkbench();
  const {
    dark,
    setDark,
    workspaces,
    sharedWorkspaces,
    selectedShared,
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
        {!selectedShared && <ToolButton
          label={sidebar ? t("收起目录") : t("展开目录")}
          className="sidebar-toggle"
          expanded={sidebar}
          controls="collection-sidebar"
          onClick={() => setSidebar(!sidebar)}
        >
          <Menu size={18} />
        </ToolButton>}
        <img src="/logo.png" alt="" width="36" height="36" />
        <Heading as="h1" weight="bold" size="3">
          MoleAPI
        </Heading>
        <span className="header-divider" />
        {(workspaces.data?.length || sharedWorkspaces.data?.length) ? (
          <Choice
            value={selectedShared ? sharedWorkspaceKey(selectedShared.owner_id, selectedShared.workspace_id) : selectedId ? ownedWorkspaceKey(selectedId) : workspaces.data?.[0] ? ownedWorkspaceKey(workspaces.data[0].id) : "none"}
            onChange={switchWorkspace}
            options={[
              ...(!workspaces.data?.length ? [{ value: "none", label: t("选择工作区") }] : []),
              ...(workspaces.data ?? []).map((w) => ({ value: ownedWorkspaceKey(w.id), label: w.name })),
              ...(sharedWorkspaces.data ?? []).map((w) => ({
                value: sharedWorkspaceKey(w.owner_id, w.workspace_id),
                label: `${w.workspace_name} · ${w.owner_username} (${w.role === "viewer" ? t("查看者") : w.role === "admin" ? t("管理员") : t("编辑者")})`,
              })),
            ]}
            label={t("工作区")}
          />
        ) : (
          <Text size="2" color="gray"> {t("API 工作台")} </Text>
        )}
        <ToolButton
          label={t("新建工作区")}
          onClick={() => openModal("new-workspace")}
        >
          <Plus size={16} />
        </ToolButton>
      </Flex>
      <Flex align="center" gap="4">
        {!selectedShared && <Button
          variant="ghost"
          color="gray"
          className="search-trigger"
          onClick={() => openModal("search")}
        >
          <Search size={16} />
          <span>{t("搜索请求")}</span>
          <kbd>⌘ K</kbd>
        </Button>}
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
            <span>{sync.data?.connected ? t("同步") : t("连接服务器")}</span>
          </Button>
        ) : (
          <Badge color="gray" variant="soft"> {t("自托管")} </Badge>
        )}
        <ToolButton
          label={dark ? t("切换为浅色") : t("切换为深色")}
          onClick={() => setDark(!dark)}
        >
          {dark ? <Sun size={17} /> : <Moon size={17} />}
        </ToolButton>
        <ToolButton label={t("工作台设置")} onClick={() => openModal("settings")}>
          <Settings size={17} />
        </ToolButton>
      </Flex>
    </header>
  );
}
