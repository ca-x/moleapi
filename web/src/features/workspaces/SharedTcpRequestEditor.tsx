import { useEffect, useRef } from "react";
import { Tabs, Text } from "@radix-ui/themes";
import { t, useLanguage } from "../../shared/i18n";
import type { RequestSpec } from "../../shared/types";
import TcpConnectionSettings from "../tcp/TcpConnectionSettings";
import TcpMessageEditor from "../tcp/TcpMessageEditor";
import RequestScripts from "../scripts/RequestScripts";

export default function SharedTcpRequestEditor(props: { request: RequestSpec; scope: string; dark: boolean; disabled: boolean; isCurrent: () => boolean; onChange: (patch: Partial<RequestSpec>) => void }) {
  useLanguage();
  const latest = useRef(props); latest.current = props;
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  function change(patch: Partial<RequestSpec>) { if (mounted.current && latest.current.scope === props.scope && !latest.current.disabled && latest.current.isCurrent()) latest.current.onChange(patch); }
  const config = props.request.protocol;
  if (config?.kind !== "tcp") return null;
  return <section aria-label={t("共享 TCP 请求设置")}><Tabs.Root defaultValue="settings"><Tabs.List>
    <Tabs.Trigger value="settings">{t("连接与分帧")}</Tabs.Trigger><Tabs.Trigger value="message">{t("已保存报文")}</Tabs.Trigger><Tabs.Trigger value="scripts">{t("脚本")}</Tabs.Trigger>
  </Tabs.List>
    <Tabs.Content value="settings"><TcpConnectionSettings request={props.request} scope={props.scope} onChange={change} disabled={props.disabled} /></Tabs.Content>
    <Tabs.Content value="message"><TcpMessageEditor message={config.message} onChange={(message) => { const current = latest.current.request.protocol; if (current?.kind === "tcp") change({ protocol: { ...current, message } }); }} dark={props.dark} disabled={props.disabled} height="180px" label={t("已保存 TCP 报文模板")} /></Tabs.Content>
    <Tabs.Content value="scripts"><Text as="p" size="1" color="gray">{t("TCP 支持连接前脚本；响应／事件脚本尚不执行，请清空响应脚本后连接。")}</Text><RequestScripts request={props.request} update={change} dark={props.dark} /></Tabs.Content>
  </Tabs.Root></section>;
}
