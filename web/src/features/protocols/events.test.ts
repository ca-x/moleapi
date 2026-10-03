import { describe, it, expect } from "vitest";
import { appendEvents, messageContent } from "./events";
import type { ProtocolEvent } from "./types";
const event = (cursor: number, text = "message"): ProtocolEvent => ({
  cursor,
  received_at: "now",
  direction: "incoming",
  message: { kind: "text", text },
});
describe("live event retention", () => {
  it("deduplicates retried batches and keeps cursor order with explicit server gaps", () => {
    const result = appendEvents([event(1), event(2)], {
      events: [event(2), event(3)],
      next_cursor: 3,
      earliest_cursor: 2,
      dropped_count: 1,
    });
    expect(result.events.map((item) => item.cursor)).toEqual([1, 2, 3]);
    expect(result.dropped).toBe(1);
  });
  it("bounds records and UTF8 bytes without silently growing browser memory", () => {
    const records = appendEvents([], {
      events: Array.from({ length: 300 }, (_, index) => event(index + 1)),
      next_cursor: 300,
      earliest_cursor: 1,
      dropped_count: 0,
    });
    expect(records.events).toHaveLength(256);
    expect(records.events[0].cursor).toBe(45);
    expect(records.dropped).toBe(44);
    const text = "鼹".repeat(1400000);
    const bytes = appendEvents([], {
      events: [event(1, text), event(2, text)],
      next_cursor: 2,
      earliest_cursor: 1,
      dropped_count: 0,
    });
    expect(bytes.events).toHaveLength(1);
    expect(bytes.events[0].cursor).toBe(2);
    expect(bytes.dropped).toBe(1);
  });
  it("preserves event data, close codes and script results for inspection", () => {
    expect(
      messageContent({
        ...event(1),
        message: {
          kind: "sse",
          event: "update",
          data: "line1\nline2",
          id: "resume",
          retry: 1000,
        },
      }),
    ).toBe("line1\nline2");
    expect(
      messageContent({
        ...event(2),
        message: { kind: "close", code: 1000, reason: "done" },
      }),
    ).toBe("1000 done");
  });
});
