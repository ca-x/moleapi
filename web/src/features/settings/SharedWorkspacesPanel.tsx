import { useQuery } from "@tanstack/react-query";
import { useRef } from "react";
import { Button, Callout, Flex, Heading, Text } from "@radix-ui/themes";
import { memberCredential as token } from "../../shared/memberApi";
import { t, useLanguage } from "../../shared/i18n";
import { liveError } from "../../shared/i18n/errors";
import { sharedWorkspaceKey } from "../workspaces/workspaceSelection";
import { fetchSharedWorkspaces } from "../workspaces/sharedWorkspacesApi";

export default function SharedWorkspacesPanel({ accountId, onOpen }: { accountId: string; onOpen: (key: string) => void }) {
  useLanguage();
  const credential = token();
  const boundary = useRef({ accountId, credential, epoch: 0 });
  if (boundary.current.accountId !== accountId || boundary.current.credential !== credential) {
    boundary.current = { accountId, credential, epoch: boundary.current.epoch + 1 };
  }
  const shared = useQuery({
    queryKey: ["shared-workspaces", accountId, boundary.current.epoch],
    queryFn: fetchSharedWorkspaces,
    enabled: !!credential,
    retry: false,
  });
  const labels = { admin: t("管理员"), editor: t("编辑者"), viewer: t("查看者") };
  if (!credential) return null;
  return <section aria-label={t("共享工作区")}><Flex direction="column" gap="3">
    <Heading size="3">{t("共享工作区")}</Heading>
    {shared.isPending && <Text role="status">{t("正在加载共享工作区…")}</Text>}
    {shared.error && <Callout.Root role="alert" color="red"><Callout.Text>{liveError(shared.error)}</Callout.Text></Callout.Root>}
    {shared.data?.length === 0 && <Text size="2" color="gray">{t("尚未加入共享工作区")}</Text>}
    {shared.data?.map((workspace) => <Flex key={`${workspace.owner_id}:${workspace.workspace_id}`} align="center" justify="between" gap="2" wrap="wrap"><Flex direction="column" gap="1"><Text weight="medium">{workspace.workspace_name}</Text><Text size="1" color="gray">{t("所有者")}: {workspace.owner_username} · {labels[workspace.role]}</Text></Flex><Button variant="soft" onClick={() => onOpen(sharedWorkspaceKey(workspace.owner_id, workspace.workspace_id))}>{t("打开共享工作区")}</Button></Flex>)}
  </Flex></section>;
}
