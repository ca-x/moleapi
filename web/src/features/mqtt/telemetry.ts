import type { ProtocolEvent } from "../protocols/types";
export interface TelemetrySample {
  cursor: number;
  time: number;
  topic: string;
  values: Record<string, number>;
}
function numbers(
  value: unknown,
  path: string[],
  result: Record<string, number>,
  budget: { left: number },
  depth = 0,
): void {
  if (--budget.left < 0 || depth > 8) return;
  if (typeof value === "number" && Number.isFinite(value)) {
    result[JSON.stringify(path)] = value;
    return;
  }
  if (!value || typeof value !== "object") return;
  for (const [key, child] of Object.entries(value)) {
    if (budget.left <= 0) return;
    numbers(child, [...path, key], result, budget, depth + 1);
  }
}
export function telemetrySamples(events: ProtocolEvent[]): TelemetrySample[] {
  const result: TelemetrySample[] = [];
  for (const event of events.slice(-256)) {
    if (
      event.message.kind !== "mqtt_message" ||
      event.direction !== "incoming" ||
      event.message.payload_redacted ||
      event.message.topic_redacted ||
      !event.message.payload_text
    )
      continue;
    try {
      const value: unknown = JSON.parse(event.message.payload_text);
      const values: Record<string, number> = Object.create(null);
      numbers(value, [], values, { left: 512 });
      const time = Date.parse(event.received_at);
      if (Object.keys(values).length && Number.isFinite(time))
        result.push({
          cursor: event.cursor,
          time,
          topic: event.message.topic,
          values,
        });
    } catch {
      /* Binary, non-JSON and malformed text do not become telemetry. */
    }
  }
  return result;
}
export function telemetryFieldLabel(field: string): string {
  const path = JSON.parse(field) as string[];
  return path.length ? path.join(" › ") : "数值";
}
