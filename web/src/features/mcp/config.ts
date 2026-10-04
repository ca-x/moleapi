import { findNodeAtLocation, parseTree } from "jsonc-parser";
import type { Pair, RequestSpec } from "../../shared/types";
import { pair } from "../../shared/model";
import { mcpConfig, object } from "./model";
import type { McpConfig } from "./types";
export interface McpHostEntry { name: string; source: string; config: McpConfig; url: string; headers: Pair[] }
const MAX_BYTES = 1024 * 1024;
function strings(value: unknown, label: string): Record<string, string> {
  if (value === undefined) return {};
  const record = object(value);
  if (!record || Object.values(record).some(value => typeof value !== "string")) throw new Error(label + "必须是字符串键值对象。");
  return record as Record<string, string>;
}
export function parseHostConfig(text: string): McpHostEntry[] {
  if (new TextEncoder().encode(text).byteLength > MAX_BYTES) throw new Error("MCP 配置超过 1 MiB。");
  const parsed = object(JSON.parse(text));
  if (!parsed) throw new Error("MCP 配置必须是 JSON 对象。");
  const tree = parseTree(text, undefined, { disallowComments: true, allowTrailingComma: false });
  if (!tree) throw new Error("MCP 配置无效。");
  const pending = [{ node: tree, depth: 0 }];
  let visited = 0;
  while (pending.length) {
    const { node, depth } = pending.pop()!;
    if (++visited > 20000 || depth > 64) throw new Error("MCP 配置结构超过限制。");
    if (node.type === "object") {
      const keys = new Set<string>();
      for (const property of node.children || []) {
        const key = String(property.children?.[0]?.value);
        if (keys.has(key)) throw new Error("MCP 配置包含重复字段：" + key);
        keys.add(key);
      }
    }
    for (const child of node.children || []) pending.push({ node: child, depth: depth+1 });
  }
  const servers = parsed.mcpServers === undefined ? { MCP: parsed } : object(parsed.mcpServers);
  if (!servers || !Object.keys(servers).length || Object.keys(servers).length > 64) throw new Error("提供 1–64 个 MCP 服务配置。");
  return Object.entries(servers).map(([name, value]) => {
    const entry = object(value);
    if (!entry) throw new Error(name + "：服务配置必须是对象。");
    const node = parsed.mcpServers === undefined ? tree : tree && findNodeAtLocation(tree, ["mcpServers", name]);
    const source = node ? text.slice(node.offset, node.offset+node.length) : JSON.stringify(entry);
    const config = mcpConfig(); config.config_source = source;
    const command = entry.command;
    if (typeof command === "string" && command.trim()) {
      if (entry.url !== undefined || (entry.type !== undefined && entry.type !== "stdio")) throw new Error(name + "：STDIO 与 HTTP 配置不能混用。");
      if (entry.args !== undefined && (!Array.isArray(entry.args) || entry.args.some(value => typeof value !== "string"))) throw new Error(name + "：args 必须是字符串数组。");
      config.transport = "stdio"; config.command = command; config.args = (entry.args || []) as string[];
      config.env = Object.entries(strings(entry.env, "env ")).map(([key, value]) => pair(key, value, true));
      return { name, source, config, url: "", headers: [] };
    }
    if (typeof entry.url !== "string" || !entry.url.trim()) throw new Error(name + "：需要 command 或 HTTP url。");
    if (entry.type !== undefined && !["http", "streamable-http"].includes(String(entry.type))) throw new Error(name + "：当前支持 Streamable HTTP 和 STDIO。");
    return { name, source, config, url: entry.url, headers: Object.entries(strings(entry.headers, "headers ")).map(([key,value]) => pair(key,value,true)) };
  });
}
/** Explicit host export includes current settings plus preserved unknown source fields. */
export function hostConfig(request: RequestSpec, name: string): string {
  if (request.protocol?.kind !== "mcp") throw new Error("当前请求不是 MCP 配置。");
  const config = request.protocol;
  let original: Record<string, unknown> = {};
  try { original = object(JSON.parse(config.config_source || "{}")) || {}; } catch { /* Current valid settings remain exportable. */ }
  const entry = { ...original };
  const env = Object.fromEntries(config.env.filter(row => row.enabled).map(row => [row.key, row.value]));
  if (config.transport === "stdio") {
    delete entry.url; delete entry.headers;
    Object.assign(entry, { type: "stdio", command: config.command, args: config.args, env });
  } else {
    delete entry.command; delete entry.args; delete entry.env;
    const headers = Object.fromEntries(request.headers.filter(row => row.enabled).map(row => [row.key,row.value]));
    if (request.auth.kind === "bearer" && request.auth.token) headers.Authorization = "Bearer " + request.auth.token;
    if (request.auth.kind === "basic") {
      const bytes = new TextEncoder().encode(request.auth.username + ":" + request.auth.password);
      if (bytes.length > 16384) throw new Error("鉴权值过长。");
      headers.Authorization = "Basic " + btoa(Array.from(bytes, value => String.fromCharCode(value)).join(""));
    }
    Object.assign(entry, { type: "http", url: request.url, headers });
  }
  return JSON.stringify({ mcpServers: { [name.trim() || "MoleAPI"]: entry } }, null, 2);
}
