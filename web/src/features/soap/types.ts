import type { Specification } from "../../shared/types";
export interface SoapConfig {
  kind: "soap";
  version: "1.1" | "1.2";
  service: string;
  port: string;
  operation: string;
  action: string;
}
export interface SoapSource {
  entry_file: string;
  files: { path: string; content: string }[];
}
export interface SoapField {
  name: string;
  namespace: string;
  type_name: string;
  optional: boolean;
  repeated: boolean;
}
export interface SoapOperation {
  name: string;
  action: string;
  template: string;
  fields: SoapField[];
  error?: string;
}
export interface SoapPort {
  name: string;
  binding: string;
  version: "1.1" | "1.2";
  address: string;
  operations: SoapOperation[];
}
export interface SoapSchema {
  services: { name: string; ports: SoapPort[] }[];
}
export interface SoapSchemaResult {
  specification: Specification;
  schema: SoapSchema;
}
export interface SoapFault {
  version: "1.1" | "1.2";
  code: string;
  reason: string;
  detail: string;
  actor?: string;
}
