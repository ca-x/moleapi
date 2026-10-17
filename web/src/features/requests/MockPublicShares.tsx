import { useEffect, useState } from "react";
import { Button, Checkbox, Flex, Select, Text, TextField } from "@radix-ui/themes";
import { api, native, token } from "../../shared/api";
import { t, useLanguage } from "../../shared/i18n";

interface MockShare {
  id: string;
  request_id: string;
  expires_at: number;
  allow_dynamic: boolean;
  stale: boolean;
}
interface GrantedShare extends MockShare { token: string }
interface ListenerStatus { active: boolean; origin: string | null; bind: string | null }

export default function MockPublicShares({ workspaceId, requestId, examples, routePath, hasSavedQuery, dirty }: {
  workspaceId: string;
  requestId: string;
  examples: { id: string; name: string; dynamic: boolean }[];
  routePath: string;
  hasSavedQuery: boolean;
  dirty: boolean;
}) {
  useLanguage();
  const [loaded, setLoaded] = useState<{ scope: string; identity: string | null; rows: MockShare[] } | null>(null);
  const [issued, setIssued] = useState<{ scope: string; identity: string | null; share: GrantedShare } | null>(null);
  const [error, setError] = useState(false);
  const [refresh, setRefresh] = useState(0);
  const [busy, setBusy] = useState("");
  const [confirm, setConfirm] = useState<{ id: string; action: "rotate" | "revoke" } | null>(null);
  const [days, setDays] = useState("7");
  const [allowDynamic, setAllowDynamic] = useState(false);
  const [listener, setListener] = useState<ListenerStatus | null>(null);
  const [listenerError, setListenerError] = useState(false);
  const [listenerBusy, setListenerBusy] = useState(false);
  const [host, setHost] = useState("127.0.0.1");
  const [port, setPort] = useState("0");
  const [allowLan, setAllowLan] = useState(false);
  const scope = `${workspaceId}\0${requestId}`;
  const identity = token();
  const path = `/api/workspaces/${encodeURIComponent(workspaceId)}/mock-shares`;
  const rows = loaded?.scope === scope && loaded.identity === identity ? loaded.rows : null;
  const currentIssued = issued?.scope === scope && issued.identity === identity ? issued.share : null;
  const expiryDays = Number(days);
  const validDays = Number.isInteger(expiryDays) && expiryDays >= 1 && expiryDays <= 90;
  const shareable = allowDynamic || examples.some(example => !example.dynamic);
  const portNumber = Number(port);
  const validPort = Number.isInteger(portNumber) && portNumber >= 0 && portNumber <= 65535;
  const listenerReady = !native || listener?.active === true;
  const listenerLoading = native && listener === null && !listenerError;
  const lanActive = listener?.active === true && listener.bind?.startsWith("0.0.0.0:") === true;

  useEffect(() => {
    if (!native) return;
    let active = true;
    void api<ListenerStatus>("/api/mock-public/listener")
      .then(result => {
        if (active) {
          setListener(result);
          if (result.active) setHost(result.bind?.startsWith("0.0.0.0:") ? "0.0.0.0" : "127.0.0.1");
        }
      })
      .catch(() => { if (active) setListenerError(true); });
    return () => { active = false; };
  }, []);

  useEffect(() => {
    setIssued(null);
    setConfirm(null);
  }, [scope, identity, dirty]);
  useEffect(() => {
    let active = true;
    setLoaded(null);
    setError(false);
    void api<MockShare[]>(path).then(result => {
      if (active && token() === identity) {
        setLoaded({ scope, identity, rows: result.filter(share => share.request_id === requestId) });
      }
    }).catch(() => { if (active && token() === identity) setError(true); });
    return () => { active = false; };
  }, [path, requestId, scope, identity, refresh, dirty]);

  const create = async () => {
    if (dirty || !listenerReady || listenerBusy || examples.length === 0 || !shareable || !validDays || busy) return;
    setBusy("create"); setError(false); setConfirm(null);
    try {
      const share = await api<GrantedShare>(path, "POST", {
        request_id: requestId,
        expires_in_days: expiryDays,
        allow_dynamic: allowDynamic,
      });
      if (token() === identity) {
        setIssued({ scope, identity, share });
        setRefresh(value => value + 1);
      }
    } catch { if (token() === identity) setError(true); }
    finally { setBusy(""); }
  };
  const change = async (share: MockShare, action: "rotate" | "revoke") => {
    if (action === "rotate" && (!listenerReady || listenerBusy)) return;
    if (confirm?.id !== share.id || confirm.action !== action) {
      setConfirm({ id: share.id, action });
      return;
    }
    setBusy(share.id); setError(false);
    try {
      if (action === "rotate") {
        const updated = await api<GrantedShare>(`${path}/${encodeURIComponent(share.id)}/rotate`, "POST");
        if (token() === identity) setIssued({ scope, identity, share: updated });
      } else {
        await api(`${path}/${encodeURIComponent(share.id)}`, "DELETE");
        if (token() === identity && currentIssued?.id === share.id) setIssued(null);
      }
      if (token() === identity) setRefresh(value => value + 1);
    } catch { if (token() === identity) setError(true); }
    finally { setBusy(""); setConfirm(null); }
  };
  const copy = async (value: string) => {
    try { await navigator.clipboard.writeText(value); }
    catch { setError(true); }
  };
  const startStop = async () => {
    if (listenerLoading || listenerBusy || busy || (!listener?.active && (!validPort || (host === "0.0.0.0" && !allowLan)))) return;
    setListenerBusy(true); setListenerError(false);
    try {
      const result = listener?.active
        ? await api<ListenerStatus>("/api/mock-public/listener/stop", "POST", {})
        : await api<ListenerStatus>("/api/mock-public/listener/start", "POST", { host, port: portNumber, allow_lan: host === "0.0.0.0" && allowLan });
      setListener(result);
      setIssued(null);
    } catch { setListenerError(true); }
    finally { setListenerBusy(false); }
  };
  const origin = native ? listener?.origin ?? "" : typeof window === "undefined" ? "" : window.location.origin;
  const issuedLinks = currentIssued ? [
    ...(routePath && !hasSavedQuery ? [{ label: t("公开匹配地址"), url: `${origin}/api/public/mocks/${encodeURIComponent(currentIssued.token)}${routePath}` }] : []),
    ...examples.filter(example => currentIssued.allow_dynamic || !example.dynamic).map(example => ({ label: `${t("示例地址")} · ${example.name}`, url: `${origin}/api/public/mock/${encodeURIComponent(currentIssued.token)}/${encodeURIComponent(example.id)}` })),
  ] : [];

  return <section aria-label={t("公开 Mock")} className="mock-public-shares">
    {native && <div className="mock-native-listener">
      <Text as="p" size="2" weight="medium">{t("本地 Mock 监听")}</Text>
      <Text as="p" size="1" color="gray">{t("监听只在手动启动后开放；停止会取消正在等待的公开调用。")}</Text>
      <Flex gap="2" align="end" wrap="wrap">
        <label className="mock-share-days"><Text size="2">{t("监听地址")}</Text><Select.Root value={host} disabled={listener?.active === true} onValueChange={value => { setHost(value); setAllowLan(false); }}><Select.Trigger aria-label={t("监听地址")} /><Select.Content><Select.Item value="127.0.0.1">{t("仅本机")}</Select.Item><Select.Item value="0.0.0.0">{t("所有接口")}</Select.Item></Select.Content></Select.Root></label>
        <label className="mock-share-days"><Text size="2">{t("端口 (0 自动分配)")}</Text><TextField.Root aria-label={t("端口 (0 自动分配)")} type="number" min="0" max="65535" step="1" value={port} onChange={event => setPort(event.target.value)} /></label>
        <Button size="2" style={{minHeight:44}} disabled={listenerLoading || listenerBusy || !!busy || (!listener?.active && (!validPort || (host === "0.0.0.0" && !allowLan)))} onClick={() => void startStop()}>{listener?.active?t("停止监听"):t("启动监听")}</Button>
      </Flex>
      {host === "0.0.0.0" && !listener?.active && <Text as="label" size="2"><Checkbox checked={allowLan} onCheckedChange={checked => setAllowLan(checked === true)} /> {t("确认允许局域网访问")}</Text>}
      {(listener?.active ? lanActive : host === "0.0.0.0") && <Text as="p" size="1" color="red">{t("所有接口会接受局域网调用；非本机访问请通过 HTTPS 代理。")}</Text>}
      {!validPort && <Text role="alert" size="1" color="red">{t("端口必须是 0–65535 的整数。")}</Text>}
      {listenerError && <Text role="alert" size="1" color="red">{t("无法读取或操作本地 Mock 监听。")}</Text>}
      {listener === null && !listenerError && <Text size="1" color="gray">{t("正在读取监听状态…")}</Text>}
      {listener?.active ? <><Text as="p" size="1">{t("监听中：{{origin}}", { origin: listener.origin ?? "" })}</Text><Text as="p" size="1">{t("实际绑定：{{bind}}", { bind: listener.bind ?? "" })}</Text></> : listener && <Text as="p" size="1" color="gray">{t("尚未启动本地监听")}</Text>}
      <Text as="p" size="1" color="gray">{t("端口 0 自动分配；重启监听后旧地址可能需要轮换。")}</Text>
    </div>}
    {dirty && <Text as="p" size="1" color="gray">{t("先保存工作区，再创建公开地址。已有地址仍可撤销。")}</Text>}
    {native && !listenerReady && <Text as="p" size="1" color="gray">{t("先启动本地监听，再创建或轮换公开地址；仍可撤销旧地址。")}</Text>}
    {examples.length === 0 && <Text as="p" size="1" color="gray">{t("先保存一个响应示例，才能创建公开地址。")}</Text>}
    {examples.length > 0 && !shareable && <Text as="p" size="1" color="gray">{t("此请求只有动态示例；创建公开地址前须允许 Mock 脚本。")}</Text>}
    <Flex gap="2" align="end" wrap="wrap">
      <label className="mock-share-days"><Text size="2">{t("有效期（天）")}</Text><TextField.Root aria-label={t("有效期（天）")} type="number" min="1" max="90" step="1" value={days} onChange={event => setDays(event.target.value)} /></label>
      <Button size="2" style={{ minHeight: 44 }} disabled={dirty || !listenerReady || listenerBusy || examples.length === 0 || !shareable || !validDays || !!busy} loading={busy === "create"} onClick={() => void create()}>{t("创建公开地址")}</Button>
    </Flex>
    {!validDays && <Text role="alert" size="1" color="red">{t("有效期必须是 1–90 天的整数。")}</Text>}
    <Text as="label" size="2"><Checkbox checked={allowDynamic} onCheckedChange={checked => setAllowDynamic(checked === true)} /> {t("允许公开地址运行 Mock 脚本")}</Text>
    {allowDynamic && <Text as="p" size="1" color="red">{t("Mock 脚本可能读取工作区数据并将其返回给公开调用者。")}</Text>}
    {error && <Text role="alert" size="1" color="red">{t("公开 Mock 操作失败，请重试。")}</Text>}
    {currentIssued && <div className="mock-share-issued">
      <Text as="p" size="2" weight="medium">{t("立即保存以下地址；关闭或重新加载后无法再次查看令牌。")}</Text>
      {hasSavedQuery && <Text as="p" size="1" color="gray">{t("此请求的匹配路由需要查询参数。请使用示例地址，或自行构造并核对查询参数。")}</Text>}
      {issuedLinks.map(link => <Flex key={link.url} gap="2" align="center" wrap="wrap">
        <Text size="1">{link.label}</Text><code>{link.url}</code>
        <Button size="2" style={{ minHeight: 44 }} variant="soft" color="gray" onClick={() => void copy(link.url)}>{t("复制地址")}</Button>
      </Flex>)}
      <Button size="2" variant="ghost" onClick={() => setIssued(null)}>{t("关闭一次性地址")}</Button>
    </div>}
    {rows === null && !error && <Text size="1" color="gray">{t("正在读取公开地址…")}</Text>}
    {rows?.length === 0 && <Text size="1" color="gray">{t("当前请求尚无公开地址")}</Text>}
    {rows && rows.length > 0 && <ul className="mock-share-list">{rows.map(share => <li key={share.id}>
      <Text size="1">{share.expires_at <= Date.now() / 1000 ? t("已过期") : share.stale ? t("请求已更改，请轮换地址。") : t("有效至 {{date}}", { date: new Date(share.expires_at * 1000).toLocaleString() })} · {share.allow_dynamic ? t("允许脚本") : t("仅静态示例")}</Text>
      <Flex gap="2" wrap="wrap">
        <Button size="2" style={{ minHeight: 44 }} variant="soft" color="gray" disabled={!!busy || dirty || !listenerReady || listenerBusy || share.expires_at <= Date.now() / 1000} onClick={() => void change(share, "rotate")}>{confirm?.id === share.id && confirm.action === "rotate" ? t("确认轮换并停用旧地址") : t("轮换地址")}</Button>
        <Button size="2" style={{ minHeight: 44 }} variant="ghost" color="red" disabled={!!busy} onClick={() => void change(share, "revoke")}>{confirm?.id === share.id && confirm.action === "revoke" ? t("确认撤销公开地址") : t("撤销地址")}</Button>
      </Flex>
    </li>)}</ul>}
  </section>;
}
