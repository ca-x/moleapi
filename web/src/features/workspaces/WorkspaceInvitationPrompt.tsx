import { useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { AlertDialog, Button, Callout, Flex } from "@radix-ui/themes";
import { toast } from "sonner";
import { api, native, token } from "../../shared/api";
import { t, useLanguage, liveTranslation } from "../../shared/i18n";
import { liveError } from "../../shared/i18n/errors";

function invitationFromUrl() {
  if (native) return "";
  return new URLSearchParams(window.location.hash.slice(1)).get("workspace_invite") ?? "";
}
function clearUrl() {
  const url = new URL(window.location.href);
  url.hash = "";
  window.history.replaceState(window.history.state, "", url);
}

export default function WorkspaceInvitationPrompt({ accountId }: { accountId: string }) {
  useLanguage();
  const client = useQueryClient();
  const credential = token();
  const origin = useRef({ accountId, credential });
  if (origin.current.accountId !== accountId || origin.current.credential !== credential) origin.current = { accountId, credential };
  const [invitation, setInvitation] = useState(invitationFromUrl);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<ReturnType<typeof liveError>>("");
  useEffect(() => { setPending(false); setError(""); setInvitation(invitationFromUrl()); }, [accountId, credential]);
  useEffect(() => {
    const changed = () => { setPending(false); setError(""); setInvitation(invitationFromUrl()); };
    window.addEventListener("hashchange", changed);
    return () => window.removeEventListener("hashchange", changed);
  }, []);
  function dismiss() { if (pending) return; clearUrl(); setInvitation(""); setError(""); }
  async function accept() {
    if (!invitation || pending || !credential) return;
    const current = origin.current;
    setPending(true); setError("");
    try {
      await api("/api/workspace-invitations/accept", "POST", { token: invitation });
      if (origin.current === current && token() === credential) {
        clearUrl(); setInvitation("");
        void client.invalidateQueries({ queryKey: ["shared-workspaces", accountId] });
        toast.success(liveTranslation("已加入共享工作区"));
      }
    } catch (cause) { if (origin.current === current && token() === credential) setError(liveError(cause)); }
    finally { if (origin.current === current && token() === credential) setPending(false); }
  }
  if (!invitation || native) return null;
  return <AlertDialog.Root open onOpenChange={(open) => { if (!open) dismiss(); }}><AlertDialog.Content>
    <AlertDialog.Title>{t("加入共享工作区")}</AlertDialog.Title>
    <AlertDialog.Description>{t("接受后，此工作区会关联到当前登录账户。请确认正在使用正确的账户。")}</AlertDialog.Description>
    {error && <Callout.Root color="red" role="alert" mt="3"><Callout.Text>{error}</Callout.Text></Callout.Root>}
    <Flex justify="end" gap="3" mt="4"><Button variant="soft" color="gray" disabled={pending} onClick={dismiss}>{t("取消")}</Button><Button loading={pending} onClick={() => void accept()}>{t("接受邀请")}</Button></Flex>
  </AlertDialog.Content></AlertDialog.Root>;
}
