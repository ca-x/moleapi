import { useState } from "react";
import { Text } from "@radix-ui/themes";
import { t, useLanguage } from "../../shared/i18n";
import type { SocketIoConfig } from "../../shared/types";
import SocketIoClient, { type SocketIoChannel } from "../socketio/SocketIoClient";

export default function SharedSocketIoSessionControls(props: { config: SocketIoConfig; channel: SocketIoChannel; dark: boolean; isCurrent: () => boolean }) {
  useLanguage();
  const [config, setConfig] = useState(() => structuredClone(props.config));
  return <section className="shared-socketio" aria-label={t("共享 Socket.IO 实时消息")}>
    <Text as="p" size="1" color="gray">{t("事件参数与监听器修改仅用于当前连接，不会自动保存到共享请求。")}</Text>
    <SocketIoClient scope={props.channel.session?.id ?? ""} config={config} onChange={(patch) => { if (props.isCurrent()) setConfig((current) => ({ ...current, ...patch })); }} dark={props.dark} isCurrent={props.isCurrent} channel={props.channel} connectionSettings={false} editorHeight="180px" />
  </section>;
}
