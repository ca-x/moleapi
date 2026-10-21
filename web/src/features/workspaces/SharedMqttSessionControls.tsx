import { useState } from "react";
import { Text } from "@radix-ui/themes";
import { t, useLanguage } from "../../shared/i18n";
import type { RequestSpec } from "../../shared/types";
import MqttClient, { type MqttChannel } from "../mqtt/MqttClient";
export default function SharedMqttSessionControls(props: { request: RequestSpec; channel: MqttChannel; dark: boolean; isCurrent: () => boolean }) {
  useLanguage();
  const [request, setRequest] = useState(() => structuredClone(props.request));
  return <section className="shared-mqtt" aria-label={t("共享 MQTT 实时消息")}>
    <Text as="p" size="1" color="gray">{t("消息与订阅修改仅用于当前连接，不会自动保存到共享请求。")}</Text>
    <MqttClient scope={props.channel.session?.id ?? ""} request={request} dark={props.dark} channel={props.channel} isCurrent={props.isCurrent} connectionSettings={false} savedMessagesEditable={false} onChange={(patch) => { if (props.isCurrent()) setRequest((current) => ({ ...current, ...patch })); }} />
  </section>;
}
