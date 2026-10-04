import { describe, expect, it } from "vitest";
import { replayRows, type Capture } from "./model";
describe("editable callback metadata", () => {
  it("retains repeated decoded query parameters and skips non-text header values", () => {
    const capture = { query: "name=one&name=two&encoded=a%2Bb", headers: [
      { name: "X-Multi", value_text: "first", value_base64: "Zmlyc3Q=" },
      { name: "X-Multi", value_text: "second", value_base64: "c2Vjb25k" },
      { name: "X-Binary", value_text: null, value_base64: "/w==" },
    ] } as Capture;
    const rows = replayRows(capture);
    expect(rows.query.map(p => [p.key, p.value])).toEqual([["name", "one"], ["name", "two"], ["encoded", "a+b"]]);
    expect(rows.headers.map(p => [p.key, p.value])).toEqual([["X-Multi", "first"], ["X-Multi", "second"]]);
    expect(new Set([...rows.query, ...rows.headers].map(p => p.id)).size).toBe(5);
  });
});
