import type { Pair, Specification, VariableUpdate } from "../../shared/types";
export interface ProtoFile {
  path: string;
  content: string;
}
export interface ProtoSource {
  kind: "proto";
  files: ProtoFile[];
  entry_files: string[];
}
export interface GrpcMethod {
  name: string;
  full_name: string;
  input_type: string;
  output_type: string;
  client_streaming: boolean;
  server_streaming: boolean;
  input_template: unknown;
}
export interface GrpcField {
  name: string;
  json_name: string;
  number: number;
  type_name: string;
  repeated: boolean;
  map: boolean;
  optional: boolean;
  oneof: string | null;
}
export interface GrpcSchema {
  messages?: { name: string; fields: GrpcField[] }[];
  enums?: { name: string; values: { name: string; number: number }[] }[];
  services: { name: string; methods: GrpcMethod[] }[];
}
export interface GrpcSchemaResult {
  specification: Specification;
  schema: GrpcSchema;
}
export interface GrpcStatus {
  code: number;
  name: string;
  message: string;
  details_base64: string;
  metadata?: Pair[];
}
export interface ReflectionResult {
  specification?: Specification;
  schema?: GrpcSchema;
  status?: GrpcStatus;
  error?: string;
  variable_updates?: VariableUpdate[];
}
export const methodMode = (method: GrpcMethod) =>
  method.client_streaming
    ? method.server_streaming
      ? "双向流"
      : "客户端流"
    : method.server_streaming
      ? "服务端流"
      : "Unary";
