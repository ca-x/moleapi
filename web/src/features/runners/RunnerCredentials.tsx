import { useEffect, useId, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertDialog, Button, Callout, Flex, Text, TextArea, TextField } from "@radix-ui/themes";
import { api, token } from "../../shared/api";
import { t, useLanguage } from "../../shared/i18n";
import { liveError } from "../../shared/i18n/errors";
import { Choice, Field } from "../../shared/ui";

interface Credential { id: string; runner_id: string; workspace_id: string; name: string; created_at: number; expires_at: number; expired: boolean }
type Request = <T>(path: string, method?: string, body?: unknown) => Promise<T>;

export default function RunnerCredentials({ accountId, runnerId, workspaceId, onClose, request = api, connectionKey }: { accountId: string; runnerId: string; workspaceId: string; onClose: () => void; request?: Request; connectionKey?: string }) {
  const instance = useId(), { language } = useLanguage(), client = useQueryClient();
  const bearer = connectionKey ?? token(), scope = JSON.stringify([accountId, runnerId, workspaceId, bearer]);
  const latest = useRef(scope); latest.current = scope;
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const current = (origin: string) => mounted.current && latest.current === origin && (connectionKey ?? token()) === bearer;
  const [name, setName] = useState(""), [days, setDays] = useState("90"), [pending, setPending] = useState(false);
  const [error, setError] = useState<ReturnType<typeof liveError>>("");
  const [secret, setSecret] = useState<{ scope: string; value: string } | null>(null);
  const [copied, setCopied] = useState(false), [selected, setSelected] = useState<Credential | null>(null);
  useEffect(() => { setName(""); setSecret(null); setCopied(false); setSelected(null); setError(""); setPending(false); }, [scope]);
  const path = `/api/runners/${encodeURIComponent(runnerId)}/credentials`;
  const queryKey = ["runner-credentials", accountId, runnerId, instance, scope];
  const credentials = useQuery({ queryKey, queryFn: () => current(scope) ? request<Credential[]>(path) : [], enabled: !!bearer, retry: false });
  async function create(event: React.FormEvent) {
    event.preventDefault(); if (pending || !current(scope)) return;
    const origin = scope; setPending(true); setError(""); setSecret(null); setCopied(false);
    try {
      const value = await request<Credential & { token: string }>(path, "POST", { workspace_id: workspaceId, name: name.trim(), expires_in_days: Number(days) });
      if (current(origin)) { setSecret({ scope: origin, value: value.token }); setName(""); void client.invalidateQueries({ queryKey }); }
    } catch (error) { if (current(origin)) setError(liveError(error)); }
    finally { if (current(origin)) setPending(false); }
  }
  async function revoke() {
    if (!selected || pending || !current(scope)) return;
    const origin = scope; setPending(true); setError("");
    try {
      await request(`${path}/${encodeURIComponent(selected.id)}`, "DELETE");
      if (current(origin)) { setSelected(null); setSecret(null); setCopied(false); void client.invalidateQueries({ queryKey }); }
    } catch (error) { if (current(origin)) setError(liveError(error)); }
    finally { if (current(origin)) setPending(false); }
  }
  async function copy() {
    if (secret?.scope !== scope || !current(scope)) return;
    const origin = scope;
    try { await navigator.clipboard.writeText(secret.value); if (current(origin)) setCopied(true); }
    catch (error) { if (current(origin)) setError(liveError(error)); }
  }
  const date = (value: number) => new Date(value * 1000).toLocaleString(language);
  return <section aria-label={t("执行器凭据")}><Flex direction="column" gap="3">
    <Flex justify="between" align="center" gap="3"><Text weight="medium">{t("执行器凭据")}</Text><Button variant="soft" color="gray" onClick={onClose}>{t("关闭凭据管理")}</Button></Flex>
    <Text size="2" color="gray">{t("凭据仅允许此执行器领取当前工作区任务。原文仅显示一次；请保存在执行器的私有文件中。")}</Text>
    {!selected && (error || credentials.error) && <Callout.Root color="red" role="alert"><Callout.Text>{error || liveError(credentials.error)}</Callout.Text></Callout.Root>}
    <form onSubmit={create}><Flex gap="3" wrap="wrap" align="end"><Field label={t("凭据名称")}><TextField.Root required maxLength={80} value={name} disabled={pending} autoComplete="off" onChange={event => setName(event.target.value)} /></Field><Choice label={t("有效期")} disabled={pending} value={days} options={[{ value: "7", label: t("7 天") }, { value: "30", label: t("30 天") }, { value: "90", label: t("90 天") }, { value: "365", label: t("365 天") }]} onChange={setDays} /><Button type="submit" loading={pending} disabled={pending || !name.trim()}>{t("创建执行器凭据")}</Button></Flex></form>
    {secret?.scope === scope && <Flex direction="column" gap="2"><Text size="2">{t("请立即复制并安全保存；关闭此视图后无法再次查看。")}</Text><TextArea aria-label={t("新执行器凭据")} readOnly spellCheck={false} value={secret.value} /><Flex gap="2"><Button variant="soft" onClick={() => void copy()}>{copied ? t("已复制") : t("复制令牌")}</Button><Button variant="ghost" onClick={() => { setSecret(null); setCopied(false); }}>{t("隐藏令牌")}</Button></Flex></Flex>}
    {credentials.data?.length === 0 && <Text size="2" color="gray">{t("当前执行器尚无凭据")}</Text>}
    {credentials.data?.filter(value => value.workspace_id === workspaceId).map(value => <Flex key={value.id} justify="between" align="center" wrap="wrap" gap="3"><Flex direction="column" gap="1"><Text weight="medium">{value.name}</Text><Text size="1" color="gray">{value.expired ? t("已过期") : t("有效")} · {t("到期时间")}: {date(value.expires_at)}</Text></Flex><Button variant="soft" color="red" disabled={pending} onClick={() => { setError(""); setSelected(value); }}>{t("撤销执行器凭据")}</Button></Flex>)}
    <AlertDialog.Root open={!!selected} onOpenChange={open => { if (!open && !pending) setSelected(null); }}><AlertDialog.Content><AlertDialog.Title>{t("撤销执行器凭据")}</AlertDialog.Title><AlertDialog.Description>{t("撤销后该凭据立即失效，正在运行的对应任务会被取消。")}</AlertDialog.Description><Text as="p" mt="2">{selected?.name}</Text>{error && <Callout.Root color="red" role="alert"><Callout.Text>{error}</Callout.Text></Callout.Root>}<Flex justify="end" gap="3" mt="4"><AlertDialog.Cancel><Button variant="soft" disabled={pending}>{t("取消")}</Button></AlertDialog.Cancel><Button color="red" loading={pending} onClick={() => void revoke()}>{t("确认撤销执行器凭据")}</Button></Flex></AlertDialog.Content></AlertDialog.Root>
  </Flex></section>;
}
