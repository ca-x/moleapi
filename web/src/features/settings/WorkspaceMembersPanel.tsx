import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertDialog, Button, Callout, Flex, Heading, Text, TextArea } from "@radix-ui/themes";
import { memberApi as api, memberCredential as token } from "../../shared/memberApi";
import { t, useLanguage } from "../../shared/i18n";
import { liveError } from "../../shared/i18n/errors";
import { Choice } from "../../shared/ui";

type Role = "admin" | "editor" | "viewer";
type Member = { member_id: string; username: string; role: Role | "owner"; revision: number };
type Invitation = { id: string; role: Role; expires_at: number; consumed_by: string | null; revoked: boolean };
type Account = { id: string; username: string };
type Target = { kind: "member" | "invitation"; id: string; label: string };

const roles: Role[] = ["admin", "editor", "viewer"];
const roleLabel = (role: Role | "owner") => ({ owner: t("所有者"), admin: t("管理员"), editor: t("编辑者"), viewer: t("查看者") })[role];

export default function WorkspaceMembersPanel({ accountId, workspaceId, ownerId: scopedOwnerId, invitationOrigin }: { accountId: string; workspaceId: string; ownerId?: string; invitationOrigin?: string }) {
  useLanguage();
  const client = useQueryClient();
  const credential = token();
  const session = !!credential && !credential.startsWith("moleapi_pat_");
  const boundary = useRef({ accountId, workspaceId, scopedOwnerId, invitationOrigin, credential, epoch: 0 });
  if (boundary.current.accountId !== accountId || boundary.current.workspaceId !== workspaceId || boundary.current.scopedOwnerId !== scopedOwnerId || boundary.current.credential !== credential || boundary.current.invitationOrigin !== invitationOrigin) {
    boundary.current = { accountId, workspaceId, scopedOwnerId, invitationOrigin, credential, epoch: boundary.current.epoch + 1 };
  }
  const scope = JSON.stringify([accountId, scopedOwnerId, workspaceId, invitationOrigin, boundary.current.epoch]);
  const latest = useRef(scope);
  latest.current = scope;
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const [role, setRole] = useState<Role>("editor");
  const [days, setDays] = useState("7");
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<ReturnType<typeof liveError>>("");
  const [secret, setSecret] = useState<{ scope: string; id: string; token: string } | null>(null);
  const [copied, setCopied] = useState(false);
  const [target, setTarget] = useState<Target | null>(null);
  const [draftRoles, setDraftRoles] = useState<Record<string, Role>>({});
  useEffect(() => { setRole("editor"); setDays("7"); setPending(false); setError(""); setSecret(null); setCopied(false); setTarget(null); setDraftRoles({}); }, [scope]);
  const baseKey = ["workspace-membership", accountId, scopedOwnerId, workspaceId, boundary.current.epoch];
  const account = useQuery({ queryKey: [...baseKey, "account"], queryFn: () => api<Account>("/api/auth/me"), enabled: session && !scopedOwnerId, retry: false });
  const ownerId = scopedOwnerId ?? account.data?.id;
  const base = ownerId ? `/api/owners/${encodeURIComponent(ownerId)}/workspaces/${encodeURIComponent(workspaceId)}` : "";
  const members = useQuery({ queryKey: [...baseKey, "members"], queryFn: () => api<Member[]>(`${base}/members`), enabled: session && !!ownerId, retry: false, refetchInterval: 3000 });
  const invitations = useQuery({ queryKey: [...baseKey, "invitations"], queryFn: () => api<Invitation[]>(`${base}/invitations`), enabled: session && !!ownerId, retry: false, refetchInterval: 3000 });
  useEffect(() => {
    if (secret && invitations.data?.some((invitation) => invitation.id === secret.id && (invitation.consumed_by || invitation.revoked || invitation.expires_at <= Date.now() / 1000))) {
      setSecret(null); setCopied(false);
    }
  }, [invitations.data, secret]);
  const current = (origin: string) => mounted.current && latest.current === origin && token() === credential;
  const refresh = () => void client.invalidateQueries({ queryKey: baseKey });

  async function createInvitation() {
    if (pending || !current(scope)) return;
    const origin = scope;
    setPending(true); setError(""); setSecret(null); setCopied(false);
    try {
      const created = await api<{ id: string; token: string }>(`${base}/invitations`, "POST", { role, expires_in_days: Number(days) });
      if (current(origin)) { setSecret({ scope: origin, id: created.id, token: created.token }); refresh(); }
    } catch (cause) { if (current(origin)) setError(liveError(cause)); }
    finally { if (current(origin)) setPending(false); }
  }
  async function updateRole(member: Member) {
    if (member.role === "owner") return;
    const next = draftRoles[member.member_id] ?? member.role;
    if (pending || next === member.role || !current(scope)) return;
    const origin = scope;
    setPending(true); setError("");
    try {
      await api(`${base}/members/${encodeURIComponent(member.member_id)}`, "PATCH", { role: next, expected_revision: member.revision });
      if (current(origin)) { setDraftRoles((old) => { const copy = { ...old }; delete copy[member.member_id]; return copy; }); refresh(); }
    } catch (cause) { if (current(origin)) setError(liveError(cause)); }
    finally { if (current(origin)) setPending(false); }
  }
  async function remove() {
    if (!target || pending || !current(scope)) return;
    const origin = scope;
    setPending(true); setError("");
    const path = target.kind === "member" ? `${base}/members/${encodeURIComponent(target.id)}` : `${base}/invitations/${encodeURIComponent(target.id)}`;
    try {
      await api(path, "DELETE");
      if (current(origin)) { setTarget(null); refresh(); }
    } catch (cause) { if (current(origin)) setError(liveError(cause)); }
    finally { if (current(origin)) setPending(false); }
  }
  async function copy() {
    if (secret?.scope !== scope || !current(scope)) return;
    const origin = scope;
    try { await navigator.clipboard.writeText(`${invitationOrigin ? new URL(invitationOrigin).origin : window.location.origin}/#workspace_invite=${encodeURIComponent(secret.token)}`); if (current(origin)) setCopied(true); }
    catch (cause) { if (current(origin)) setError(liveError(cause)); }
  }

  if (!session) return null;
  return <section aria-label={t("工作区成员")}><Flex direction="column" gap="3">
    <Heading size="3">{t("工作区成员")}</Heading>
    <Text size="2" color="gray">{t("邀请链接仅在创建时显示。成员使用自己的账户访问这个工作区。")}</Text>
    {(error || account.error || members.error || invitations.error) && <Callout.Root color="red" role="alert"><Callout.Text>{error || liveError(account.error || members.error || invitations.error)}</Callout.Text></Callout.Root>}
    <Flex gap="2" wrap="wrap" align="end">
      <Choice<Role> label={t("邀请角色")} value={role} onChange={setRole} disabled={pending} options={roles.map((value) => ({ value, label: roleLabel(value) }))} />
      <Choice label={t("邀请有效期")} value={days} onChange={setDays} disabled={pending} options={["1", "7", "14"].map((value) => ({ value, label: t("{{value0}} 天", { value0: value }) }))} />
      <Button onClick={() => void createInvitation()} loading={pending} disabled={pending || !ownerId}>{t("创建邀请链接")}</Button>
    </Flex>
    {secret?.scope === scope && <Flex direction="column" gap="2">
      <Text size="2">{t("请立即复制邀请链接；关闭此视图后无法再次查看。")}</Text>
      <TextArea aria-label={t("新邀请链接")} readOnly value={`${invitationOrigin ? new URL(invitationOrigin).origin : window.location.origin}/#workspace_invite=${encodeURIComponent(secret.token)}`} spellCheck={false} />
      <Flex gap="2"><Button variant="soft" onClick={() => void copy()}>{copied ? t("已复制") : t("复制邀请链接")}</Button><Button variant="ghost" onClick={() => { setSecret(null); setCopied(false); }}>{t("隐藏邀请链接")}</Button></Flex>
    </Flex>}
    {members.isPending && <Text role="status">{t("正在加载成员…")}</Text>}
    {members.data?.map((member) => <Flex key={member.member_id} align="center" justify="between" gap="2" wrap="wrap">
      <Flex direction="column" gap="1"><Text weight="medium">{member.username}</Text><Text size="1" color="gray">{roleLabel(member.role)}</Text></Flex>
      {member.role !== "owner" && <Flex gap="2" align="center" wrap="wrap">
        <Choice<Role> label={`${member.username} ${t("成员角色")}`} value={draftRoles[member.member_id] ?? member.role} onChange={(value) => setDraftRoles((old) => ({ ...old, [member.member_id]: value }))} disabled={pending} options={roles.map((value) => ({ value, label: roleLabel(value) }))} />
        <Button variant="soft" disabled={pending || (draftRoles[member.member_id] ?? member.role) === member.role} onClick={() => void updateRole(member)}>{t("更新角色")}</Button>
        <Button variant="soft" color="red" disabled={pending} onClick={() => setTarget({ kind: "member", id: member.member_id, label: member.username })}>{t("移除成员")}</Button>
      </Flex>}
    </Flex>)}
    <Heading size="2">{t("待接受邀请")}</Heading>
    {invitations.data?.filter((invitation) => !invitation.consumed_by && !invitation.revoked && invitation.expires_at > Date.now() / 1000).length === 0 && <Text size="2" color="gray">{t("没有待接受的邀请")}</Text>}
    {invitations.data?.filter((invitation) => !invitation.consumed_by && !invitation.revoked && invitation.expires_at > Date.now() / 1000).map((invitation) => <Flex key={invitation.id} align="center" justify="between" gap="2" wrap="wrap"><Text size="2">{roleLabel(invitation.role)} · {new Date(invitation.expires_at * 1000).toLocaleString()}</Text><Button variant="soft" color="red" disabled={pending} onClick={() => setTarget({ kind: "invitation", id: invitation.id, label: roleLabel(invitation.role) })}>{t("撤销邀请")}</Button></Flex>)}
    <AlertDialog.Root open={!!target} onOpenChange={(open) => { if (!open && !pending) setTarget(null); }}><AlertDialog.Content><AlertDialog.Title>{target?.kind === "member" ? t("移除成员") : t("撤销邀请")}</AlertDialog.Title><AlertDialog.Description>{target?.kind === "member" ? t("移除后，该成员将立即失去此工作区的访问权限。") : t("撤销后，此链接将无法再被接受。")}</AlertDialog.Description><Text as="p" mt="2">{target?.label}</Text><Flex justify="end" gap="3" mt="4"><AlertDialog.Cancel><Button variant="soft" color="gray" disabled={pending}>{t("取消")}</Button></AlertDialog.Cancel><Button color="red" loading={pending} onClick={() => void remove()}>{t("确认")}</Button></Flex></AlertDialog.Content></AlertDialog.Root>
  </Flex></section>;
}
