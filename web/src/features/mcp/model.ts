import type { McpConfig } from "./types";
export function mcpConfig(): McpConfig {
  return { kind: "mcp", transport: "http", command: "", args: [], env: [], operation: "tools/call", name: "", arguments_source: "{}", uri: "" };
}
/** SDK content stays data: URI, HTML and unknown blocks never trigger network access. */
export function object(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : null;
}
export function resultContents(result: unknown): Record<string, unknown>[] {
  const value = object(result);
  if (!value) return [];
  const blocks: unknown[] = [];
  if (Array.isArray(value.content)) blocks.push(...value.content);
  if (Array.isArray(value.contents)) {
    for (const resource of value.contents) blocks.push({ type: "resource", resource });
  }
  if (Array.isArray(value.messages)) {
    for (const message of value.messages) {
      const entry = object(message);
      if (entry?.content) blocks.push(entry.content);
    }
  }
  return blocks.map(object).filter((value): value is Record<string, unknown> => !!value);
}
export function callbackTemplate(method: string): string {
  if (method === "elicitation/create") return '{\n  "action": "accept",\n  "content": {}\n}';
  if (method === "sampling/createMessage") return '{\n  "role": "assistant",\n  "content": { "type": "text", "text": "" },\n  "model": "manual",\n  "stopReason": "endTurn"\n}';
  if (method === "roots/list") return '{\n  "roots": []\n}';
  return "{}";
}
