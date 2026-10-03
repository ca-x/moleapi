import type {
  MqttConfig,
  MqttMessage,
  MqttProperties,
  MqttSubscription,
} from "./types";
export const mqttProperties = (): MqttProperties => ({
  payload_format_indicator: null,
  message_expiry_interval: null,
  response_topic: null,
  correlation_data_base64: null,
  content_type: null,
  user_properties: [],
});
export const mqttMessage = (): MqttMessage => ({
  topic: "",
  payload_source: "",
  encoding: "text",
  qos: 0,
  retain: false,
  topic_secret: false,
  payload_secret: false,
  properties: mqttProperties(),
});
export const mqttSubscription = (filter = ""): MqttSubscription => ({
  filter,
  qos: 0,
  enabled: true,
  filter_secret: false,
  no_local: false,
  retain_as_published: false,
  retain_handling: 0,
  subscription_identifier: null,
  user_properties: [],
});
export const mqttConfig = (): MqttConfig => ({
  kind: "mqtt",
  version: "5",
  client_id: "",
  clean_start: true,
  keep_alive_secs: 60,
  session_expiry_interval: 0,
  reconnect: { enabled: false, max_attempts: 3, delay_ms: 1000 },
  subscriptions: [],
  user_properties: [],
  will: null,
  message: mqttMessage(),
  saved_messages: [],
});
export function hasV5Properties(value: MqttProperties): boolean {
  return (
    value.payload_format_indicator !== null ||
    value.message_expiry_interval !== null ||
    !!value.response_topic ||
    !!value.correlation_data_base64 ||
    !!value.content_type ||
    value.user_properties.length > 0
  );
}
export function hasV5Settings(value: MqttConfig): boolean {
  return (
    value.session_expiry_interval !== 0 ||
    value.user_properties.length > 0 ||
    !!value.will?.delay_interval ||
    (!!value.will && hasV5Properties(value.will.message.properties)) ||
    hasV5Properties(value.message.properties) ||
    value.saved_messages.some((entry) =>
      hasV5Properties(entry.message.properties),
    ) ||
    value.subscriptions.some(
      (entry) =>
        entry.no_local ||
        entry.retain_as_published ||
        entry.retain_handling !== 0 ||
        entry.subscription_identifier !== null ||
        entry.user_properties.length > 0,
    )
  );
}
export function clearV5Settings(value: MqttConfig): MqttConfig {
  return {
    ...value,
    session_expiry_interval: 0,
    user_properties: [],
    will: value.will
      ? {
          ...value.will,
          delay_interval: 0,
          message: { ...value.will.message, properties: mqttProperties() },
        }
      : null,
    message: { ...value.message, properties: mqttProperties() },
    saved_messages: value.saved_messages.map((entry) => ({
      ...entry,
      message: { ...entry.message, properties: mqttProperties() },
    })),
    subscriptions: value.subscriptions.map((entry) => ({
      ...entry,
      no_local: false,
      retain_as_published: false,
      retain_handling: 0,
      subscription_identifier: null,
      user_properties: [],
    })),
  };
}

export function mqttSavedLabel(
  name: string,
  previous: string | undefined,
  message: MqttMessage,
): string {
  const label =
    name.trim() ||
    previous ||
    (!message.topic_secret ? message.topic : "") ||
    "MQTT 消息";
  return message.topic_secret && message.topic && label.includes(message.topic)
    ? label.replaceAll(message.topic, "[私密 Topic]")
    : label;
}
