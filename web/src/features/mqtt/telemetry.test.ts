import { describe, expect, it } from "vitest";
import type { ProtocolEvent } from "../protocols/types";
import { telemetrySamples } from "./telemetry";
const event = (
  payload: string,
  patch: Record<string, unknown> = {},
): ProtocolEvent => ({
  cursor: 1,
  received_at: "2026-10-03T00:00:00Z",
  direction: "incoming",
  message: {
    kind: "mqtt_message",
    topic: "sensor",
    payload_base64: "",
    payload_text: payload,
    qos: 0,
    retain: false,
    duplicate: false,
    packet_id: null,
    properties: {},
    topic_redacted: false,
    payload_redacted: false,
    properties_redacted: false,
    ...patch,
  },
});
describe("bounded MQTT telemetry", () => {
  it("excludes private/non-JSON/non-numeric and outgoing data", () => {
    expect(
      telemetrySamples([
        event('{"v":2}', { payload_redacted: true }),
        event('{"v":2}', { topic_redacted: true }),
        event("not JSON"),
        event('{"v":"text"}'),
        { ...event('{"v":2}'), direction: "outgoing" },
      ]),
    ).toEqual([]);
  });
  it("keeps exact distinct field paths rather than merging dotted JSON keys", () => {
    const values = telemetrySamples([
      event('{"a.b":1,"a":{"b":2},"list":[3]}'),
    ])[0].values;
    expect(values['["a.b"]']).toBe(1);
    expect(values['["a","b"]']).toBe(2);
    expect(values['["list","0"]']).toBe(3);
  });
  it("caps traversal and retained samples for large broker payloads", () => {
    const value = JSON.stringify(
      Object.fromEntries(
        Array.from({ length: 1000 }, (_, index) => ["value" + index, index]),
      ),
    );
    expect(
      Object.keys(telemetrySamples([event(value)])[0].values).length,
    ).toBeLessThanOrEqual(512);
    expect(
      telemetrySamples(
        Array.from({ length: 300 }, (_, index) => ({
          ...event('{"v":2}'),
          cursor: index,
        })),
      ),
    ).toHaveLength(256);
  });
});
