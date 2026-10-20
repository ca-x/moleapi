import { useEffect, useRef } from "react";
import { t, useLanguage } from "../../shared/i18n";
import type { RequestSpec, SocketIoConfig } from "../../shared/types";
import SocketIoClient from "../socketio/SocketIoClient";

export default function SharedSocketIoRequestEditor(props: { scope: string; request: RequestSpec; dark: boolean; disabled: boolean; isCurrent: () => boolean; onChange: (patch: Partial<RequestSpec>) => void }) {
  useLanguage();
  const latest = useRef(props); latest.current = props;
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  function change(patch: Partial<SocketIoConfig>) {
    const current = latest.current, config = current.request.protocol;
    if (mounted.current && current.scope === props.scope && !current.disabled && current.isCurrent() && config?.kind === "socketio") current.onChange({ protocol: { ...config, ...patch } });
  }
  const config = props.request.protocol;
  if (config?.kind !== "socketio") return null;
  return <section className="shared-socketio" aria-label={t("共享 Socket.IO 请求设置")}><SocketIoClient scope={props.scope} config={config} dark={props.dark} disabled={props.disabled} isCurrent={props.isCurrent} onChange={change} editorHeight="180px" /></section>;
}
