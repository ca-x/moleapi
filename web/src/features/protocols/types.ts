import type {
  Pair,
  RequestUpdate,
  TestResult,
  VariableUpdate,
} from "../../shared/types";
export type { ProtocolConfig } from "../../shared/types";
export type SessionState = "connecting" | "open" | "closed" | "error";
export interface ProtocolSession {
  id: string;
  workspace_id: string;
  request_id: string;
  protocol: "sse" | "websocket" | "graphql" | "grpc";
  client_half_closed?: boolean;
  url: string;
  state: SessionState;
  reason: string | null;
  created_at: string;
  updated_at: string;
  received_bytes: number;
  sent_bytes: number;
  event_count: number;
  handshake: { status: number; headers: Pair[] } | null;
  variable_updates?: VariableUpdate[];
  request_updates?: RequestUpdate[];
}
export type ProtocolMessage =
  | { kind: "grpc_message"; message: unknown }
  | { kind: "grpc_metadata"; phase: "headers" | "trailers"; metadata: Pair[] }
  | {
      kind: "grpc_status";
      code: number;
      name: string;
      message: string;
      details_base64: string;
      metadata: Pair[];
    }
  | {
      kind: "graphql_next" | "graphql_error" | "graphql_complete";
      operation_id: string;
      payload: unknown;
    }
  | {
      kind: "sse";
      event: string;
      data: string;
      id: string;
      retry: number | null;
    }
  | { kind: "text"; text: string }
  | { kind: "binary" | "ping" | "pong"; base64: string }
  | { kind: "close"; code: number | null; reason: string }
  | { kind: "state"; state: SessionState; reason: string | null }
  | { kind: "script_log"; level: string; message: string }
  | { kind: "script_test"; test: TestResult };
export interface ProtocolEvent {
  cursor: number;
  received_at: string;
  direction: "incoming" | "outgoing" | "system";
  message: ProtocolMessage;
}
export interface EventBatch {
  events: ProtocolEvent[];
  next_cursor: number;
  earliest_cursor: number;
  dropped_count: number;
}
export type SendMessage =
  | { kind: "grpc_message"; message_source: string }
  | { kind: "grpc_half_close" }
  | { kind: "text"; text: string }
  | { kind: "binary" | "ping"; base64: string };
