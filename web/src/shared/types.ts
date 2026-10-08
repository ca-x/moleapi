import type {OAuth2Auth} from "../features/authentication/oauth2/types";
import type { DataConfig } from "../features/data/types";
import type { TcpConfig } from "../features/tcp/types";
import type { A2aConfig } from "../features/a2a/types";
import type { McpConfig } from "../features/mcp/types";
import type { SoapConfig, SoapFault } from "../features/soap/types";
import type { MqttConfig } from "../features/mqtt/types";
export interface SocketIoConfig {
  kind: "socketio";
  namespace: string;
  path: string;
  auth_source: string;
  listeners: string[];
  event: string;
  arguments_source: string;
  attachments_base64: string[];
  request_ack: boolean;
  ack_timeout_ms: number;
}
export interface GrpcConfig {
  kind: "grpc";
  service: string;
  method: string;
  message_source: string;
}
export interface GraphQLConfig {
  kind: "graphql";
  document: string;
  variables: Record<string, unknown>;
  variables_source?: string | null;
  operation_name?: string | null;
  connection_params: Record<string, unknown>;
  subscription_url?: string | null;
}
export type ProtocolConfig =
  | DataConfig
  | TcpConfig
  | { kind: "http" | "sse" | "websocket" }
  | GraphQLConfig
  | GrpcConfig
  | SocketIoConfig
  | MqttConfig
  | SoapConfig
  | McpConfig
  | A2aConfig;
export interface Pair {
  id: string;
  key: string;
  value: string;
  enabled: boolean;
  secret?: boolean;
  local_value?: string | null;
}
export interface ApiKeyAuth {name:string;value:string;location:"header"|"query"}
export interface JwtAuth {algorithm:string;key:string;key_base64:boolean;claims_source:string;kid:string;name:string;prefix:string;location:"header"|"query";add_time_claims:boolean;ttl_seconds:number}
export interface AwsAuth {access_key:string;secret_key:string;session_token:string;region:string;service:string;location:"header"|"query";expires_seconds:number;unsigned_payload:boolean}
export interface HawkAuth {id:string;key:string;algorithm:string;nonce:string;timestamp:string;ext:string;app:string;delegation:string;user:string;include_payload_hash:boolean}
export interface OAuth1Auth {consumer_key:string;consumer_secret:string;token:string;token_secret:string;private_key:string;algorithm:string;location:"header"|"query"|"body"|"automatic";realm:string;nonce:string;timestamp:string;callback:string;verifier:string;include_version:boolean;include_body_hash:boolean;include_empty_params:boolean}
export interface Auth {
  oauth1?:OAuth1Auth;
  hawk?:HawkAuth;
  aws?:AwsAuth;
  oauth2?:OAuth2Auth;
  api_key?:ApiKeyAuth;
  jwt?:JwtAuth;
  kind: "inherit" | "none" | "bearer" | "basic" | "apikey" | "jwt" | "digest" | "oauth2" | "aws" | "hawk" | "oauth1";
  token: string;
  username: string;
  password: string;
}
export interface Assertion {
  id: string;
  name: string;
  kind: "status" | "duration" | "contains" | "json";
  target: string;
  expected: string;
}
export interface Example {
  id: string;
  name: string;
  status: number;
  headers: Pair[];
  body: string;
}
export interface RequestSpec {
  protocol?: ProtocolConfig;
  id: string;
  name: string;
  method: string;
  url: string;
  description: string;
  query: Pair[];
  headers: Pair[];
  body_kind: "none" | "json" | "text" | "form" | "binary" | "multipart";
  body: string;
  auth: Auth;
  timeout_ms: number;
  follow_redirects: boolean;
  verify_tls: boolean;
  assertions: Assertion[];
  examples: Example[];
  specification_id?: string | null;
  operation_id?: string | null;
  pre_request_script?: string;
  post_response_script?: string;
}
export interface Collection {
  variables_enabled?:boolean | null;
  parent_id?: string | null;
  auth?: Auth | null;
  variables?: Pair[];
  pre_request_script?: string;
  post_response_script?: string;
  id: string;
  name: string;
  description: string;
  requests: RequestSpec[];
}
export interface Environment {
  id: string;
  name: string;
  variables: Pair[];
}
export interface Specification {
  id: string;
  name: string;
  kind: string;
  source: string;
  dialect: string;
}
export interface WorkspaceData {
  auth?: Auth | null;
  global_variables?: Pair[];
  pre_request_script?: string;
  post_response_script?: string;
  specifications?: Specification[];
  schema_version: 1;
  collections: Collection[];
  environments: Environment[];
  active_environment_id: string | null;
}
export interface Workspace {
  id: string;
  name: string;
  revision: number;
  updated_at: string;
  data: WorkspaceData;
}
export interface TestResult {
  id: string;
  name: string;
  passed: boolean;
  actual: string;
  expected: string;
}
export interface ScriptLog {
  level: string;
  message: string;
}
export interface VariableUpdate {
  scope: string;
  key: string;
  value?: string;
}
export interface RequestUpdate {
  field: string;
  value: string;
}
export interface ApiResponse {
  request_updates?: RequestUpdate[];
  logs?: ScriptLog[];
  variable_updates?: VariableUpdate[];
  soap_fault?: SoapFault | null;
  status: number;
  status_text: string;
  headers: Pair[];
  body: string;
  body_base64?: string;
  elapsed_ms: number;
  size_bytes: number;
  truncated: boolean;
  url: string;
  tests: TestResult[];
}
export interface HistoryEntry {
  id: string;
  workspace_id: string;
  request_id: string;
  request_name: string;
  method: string;
  url: string;
  status: number;
  elapsed_ms: number;
  size_bytes: number;
  created_at: string;
  response: ApiResponse;
}
export interface AuthStatus {
  mode: "server" | "desktop";
  setup_required: boolean;
  registration_enabled: boolean;
}
export interface SyncStatus {
  connected: boolean;
  server_url?: string;
  username?: string;
}
export interface SyncResult {
  status: "synced" | "conflict";
  workspace: Workspace;
  remote?: Workspace;
  message: string;
}
export interface RunResult {
  results: {
    request_id: string;
    request_name: string;
    response?: ApiResponse;
    error?: string;
  }[];
  passed: number;
  failed: number;
  elapsed_ms: number;
}
export interface ImportResult {
  name: string;
  data: WorkspaceData;
  warnings: string[];
}
export interface ExportResult {
  filename: string;
  content: string;
  mime: string;
}
