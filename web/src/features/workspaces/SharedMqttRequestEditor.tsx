import { t, useLanguage } from "../../shared/i18n";
import type { RequestSpec } from "../../shared/types";
import MqttClient from "../mqtt/MqttClient";
export default function SharedMqttRequestEditor(props: { scope: string; request: RequestSpec; dark: boolean; disabled: boolean; isCurrent: () => boolean; onChange: (patch: Partial<RequestSpec>) => void }) {
  useLanguage();
  return <section className="shared-mqtt" aria-label={t("共享 MQTT 请求设置")}><MqttClient {...props} networkSettings={false} /></section>;
}
