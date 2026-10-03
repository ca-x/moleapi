export interface MqttProperty {
  key: string;
  value: string;
  secret: boolean;
}
export interface MqttProperties {
  payload_format_indicator: 0 | 1 | null;
  message_expiry_interval: number | null;
  response_topic: string | null;
  correlation_data_base64: string | null;
  content_type: string | null;
  user_properties: MqttProperty[];
}
export interface MqttMessage {
  topic: string;
  payload_source: string;
  encoding: "text" | "json" | "base64";
  qos: 0 | 1 | 2;
  retain: boolean;
  topic_secret: boolean;
  payload_secret: boolean;
  properties: MqttProperties;
}
export interface MqttSubscription {
  filter: string;
  qos: 0 | 1 | 2;
  enabled: boolean;
  filter_secret: boolean;
  no_local: boolean;
  retain_as_published: boolean;
  retain_handling: 0 | 1 | 2;
  subscription_identifier: number | null;
  user_properties: MqttProperty[];
}
export interface MqttConfig {
  kind: "mqtt";
  version: "3.1.1" | "5";
  client_id: string;
  clean_start: boolean;
  keep_alive_secs: number;
  session_expiry_interval: number;
  reconnect: { enabled: boolean; max_attempts: number; delay_ms: number };
  subscriptions: MqttSubscription[];
  user_properties: MqttProperty[];
  will: { message: MqttMessage; delay_interval: number } | null;
  message: MqttMessage;
  saved_messages: { id: string; name: string; message: MqttMessage }[];
}
