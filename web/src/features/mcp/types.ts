import type { Pair } from "../../shared/types";
export type McpOperation = "tools/call" | "resources/read" | "prompts/get";
export type McpMethod = McpOperation | "refresh" | "resources/subscribe" | "resources/unsubscribe";
export interface McpConfig {
  kind: "mcp";
  transport: "http" | "stdio";
  command: string;
  args: string[];
  env: Pair[];
  operation: McpOperation;
  name: string;
  arguments_source: string;
  uri: string;
  config_source?: string | null;
}
export interface McpCapability {
  name?: string;
  title?: string;
  description?: string;
  uri?: string;
  uriTemplate?: string;
  inputSchema?: Record<string, unknown>;
  arguments?: { name: string; description?: string; required?: boolean }[];
  [key: string]: unknown;
}
export interface McpCapabilities {
  tools: McpCapability[];
  resources: McpCapability[];
  resource_templates: McpCapability[];
  prompts: McpCapability[];
}
export type McpMessage =
  | { kind: "mcp_initialized"; info: Record<string, unknown> }
  | ({ kind: "mcp_capabilities" } & McpCapabilities)
  | { kind: "mcp_result"; request_id: string; method: string; result: unknown }
  | { kind: "mcp_error"; request_id: string; method: string; error: unknown }
  | { kind: "mcp_notification"; method: string; params: unknown }
  | { kind: "mcp_callback"; callback_id: string; method: string; params: unknown };
export type McpCommand =
  | { kind: "mcp_request"; request_id: string; method: McpMethod; name?: string; uri?: string; arguments_source?: string }
  | { kind: "mcp_callback"; callback_id: string; result?: unknown; error?: { code: number; message: string } }
  | { kind: "mcp_cancel"; request_id: string };
