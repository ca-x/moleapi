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
export interface OAuth1Grant {request_token_url:string;authorization_url:string;access_token_url:string;callback_url:string;request_params:Pair[];access_params:Pair[]}
export interface OAuth1Auth {grant?:OAuth1Grant|null;token_id?:string|null;consumer_key:string;consumer_secret:string;token:string;token_secret:string;private_key:string;algorithm:string;location:"header"|"query"|"body"|"automatic";realm:string;nonce:string;timestamp:string;callback:string;verifier:string;include_version:boolean;include_body_hash:boolean;include_empty_params:boolean}
export interface NtlmAuth {domain:string;workstation:string;channel_binding:boolean}
export interface EdgeGridAuth {access_token:string;client_token:string;client_secret:string;base_url:string;headers_to_sign:string[];nonce:string;timestamp:string;max_body_bytes:number}
export interface AsapAuth {algorithm:string;private_key:string;key_id:string;issuer:string;audience:string[];subject:string;ttl_seconds:number;claims_source:string}
export interface Auth {
  asap?:AsapAuth;
  edgegrid?:EdgeGridAuth;
  ntlm?:NtlmAuth;
  oauth1?:OAuth1Auth;
  hawk?:HawkAuth;
  aws?:AwsAuth;
  oauth2?:OAuth2Auth;
  api_key?:ApiKeyAuth;
  jwt?:JwtAuth;
  kind: "inherit" | "none" | "bearer" | "basic" | "apikey" | "jwt" | "digest" | "oauth2" | "aws" | "hawk" | "oauth1" | "ntlm" | "edgegrid" | "asap";
  token: string;
  username: string;
  password: string;
}
export interface Assertion {
  id: string;
  name: string;
  kind: "status" | "duration" | "contains" | "json" | "header" | "regex" | "jsonpath" | "xpath" | "schema";
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
export interface RequestNetwork {
  http_mode: "http1" | "auto" | "http2_prior_knowledge";
  proxy: {enabled:boolean;url:string;username:string;password:string;bypass:string};
  built_in_roots:boolean;
  ca_pem:string;
  identity:{enabled:boolean;format:"pem"|"pkcs12";certificate_pem:string;key_pem:string;pkcs12_base64:string;password:string;alias:string};
  dns:{hostname:string;addresses:string[]}[];
  connect_timeout_ms:number;
}
export interface RequestSpec {
  extractions?:Extraction[];
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
  network?: RequestNetwork;
  assertions: Assertion[];
  examples: Example[];
  specification_id?: string | null;
  operation_id?: string | null;
  pre_request_script?: string;
  post_response_script?: string;
}
export interface Extraction {id:string;name:string;kind:"body"|"header"|"json"|"jsonpath"|"xpath"|"regex";target:string;scope:"temporary"|"environment"|"collection"|"project";key:string;enabled:boolean;required:boolean}
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
export type ScenarioTarget={action:"step";step_id:string}|{action:"stop"};
export interface ScenarioStep {id:string;request_id:string;name:string;group:string;enabled:boolean;condition?:string;repeat?:number;on_true?:ScenarioTarget;on_false?:ScenarioTarget}
export interface ScenarioParallel {id:string;name:string;step_ids:string[];concurrency:number}
export interface Scenario {id:string;name:string;description:string;collection_id:string;steps:ScenarioStep[];parallel?:ScenarioParallel[]}
export interface WorkspaceData {
  scenarios?:Scenario[];
  datasets?:SavedDataset[];
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
export interface SavedDataset {id:string;name:string;description:string;secret:boolean;source?:{format:"csv"|"json";source:string}}
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
  skipped?:boolean;
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
  parallel_variable_updates?:({collection_id:string}&VariableUpdate)[];
  scenario_id?:string;
  skipped?:number;
  executed_steps?:number;
  iteration_count?:number;
  completed_iterations?:number;
  iterations?:{iteration:number;passed:number;failed:number;elapsed_ms:number;script_stopped?:boolean;scenario_stopped?:boolean}[];
  cancelled?:boolean;
  stopped_reason?:string|null;
  omitted_responses?:number;
  results: {
    parallel_id?:string;
    parallel_name?:string;
    variable_updates_applied?:boolean;
    condition_skipped?:boolean;
    step_repeat_index?:number;
    step_id?:string;
    step_name?:string;
    step_group?:string;
    request_id: string;
    request_name: string;
    collection_id?:string;
    iteration?:number;
    status?:number;
    elapsed_ms?:number;
    response_omitted?:boolean;
    response?: ApiResponse;
    error?: string;
  }[];
  passed: number;
  failed: number;
  elapsed_ms: number;
}
export interface RunOptions {scenario_id?:string;iterations?:number;dataset?:{format:"csv"|"json";source:string};dataset_id?:string}
export interface ImportResult {
  name: string;
  data: WorkspaceData;
  warnings: string[];
}
export interface ExportResult {
  warnings?:string[];
  filename: string;
  content: string;
  mime: string;
}
