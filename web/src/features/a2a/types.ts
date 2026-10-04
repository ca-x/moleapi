import type { Specification } from "../../shared/types";
export type A2aDialect = "0.3" | "1.0";
export type A2aTransport = "jsonrpc" | "http-json";
export type A2aMethod = "message/send" | "message/stream" | "tasks/list" | "tasks/get" | "tasks/cancel" | "tasks/resubscribe" | "tasks/pushNotificationConfig/set" | "tasks/pushNotificationConfig/get" | "tasks/pushNotificationConfig/list" | "tasks/pushNotificationConfig/delete";
export interface A2aConfig {
  kind: "a2a";
  dialect: A2aDialect;
  transport: A2aTransport;
  operation: A2aMethod;
  params_source: string;
  card_source?: string | null;
  interface_url?: string | null;
}
export interface A2aInterface { url: string; transport: string; version: string; supported: boolean }
export interface A2aCardResult { specification: Specification; card: Record<string, unknown>; dialect: A2aDialect; interfaces: A2aInterface[]; warnings: string[] }
export type A2aMessage =
  | {kind:"a2a_ready";dialect:A2aDialect;transport:A2aTransport}
  | {kind:"a2a_result";request_id:string;method:string;result:unknown}
  | {kind:"a2a_stream";request_id:string;method:string;result:unknown}
  | {kind:"a2a_error";request_id:string;method:string;error:unknown}
  | {kind:"a2a_finished";request_id:string;method:string};
export type A2aCommand =
  | {kind:"a2a_request";request_id:string;method:A2aMethod;params_source:string}
  | {kind:"a2a_stop";request_id:string};
