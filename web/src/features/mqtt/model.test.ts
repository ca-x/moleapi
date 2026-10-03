import { describe, expect, it } from "vitest";
import {
  clearV5Settings,
  hasV5Settings,
  mqttConfig,
  mqttProperties,
  mqttSubscription,
  mqttSavedLabel,
} from "./model";
describe("canonical MQTT drafts", () => {
  it("clearing unsupported v5 options retains payload, topics, privacy and saved-message identity", () => {
    const value = mqttConfig();
    value.version = "3.1.1";
    value.session_expiry_interval = 300;
    value.user_properties = [{ key: "tenant", value: "private", secret: true }];
    value.message = {
      ...value.message,
      topic: "sensor/demo",
      payload_source: '{"count":9007199254740993}',
      qos: 2,
      retain: true,
      payload_secret: true,
      properties: { ...mqttProperties(), content_type: "application/json" },
    };
    value.subscriptions = [
      {
        ...mqttSubscription("sensor/#"),
        no_local: true,
        subscription_identifier: 7,
      },
    ];
    value.saved_messages = [
      { id: "saved", name: "Sample", message: structuredClone(value.message) },
    ];
    expect(hasV5Settings(value)).toBe(true);
    const cleared = clearV5Settings(value);
    expect(hasV5Settings(cleared)).toBe(false);
    expect(cleared.message).toMatchObject({
      topic: "sensor/demo",
      payload_source: '{"count":9007199254740993}',
      qos: 2,
      retain: true,
      payload_secret: true,
    });
    expect(cleared.subscriptions[0].filter).toBe("sensor/#");
    expect(cleared.saved_messages[0].id).toBe("saved");
    expect(value.session_expiry_interval).toBe(300);
  });
});

it("generated public-topic labels do not remain exposed after marking the saved message private", () => {
  const message = {
    ...mqttConfig().message,
    topic: "customer/acme",
    topic_secret: true,
  };
  expect(mqttSavedLabel("customer/acme", "customer/acme", message)).toBe(
    "[私密 Topic]",
  );
  expect(mqttSavedLabel("", "", message)).toBe("MQTT 消息");
  expect(message.topic).toBe("customer/acme");
});
