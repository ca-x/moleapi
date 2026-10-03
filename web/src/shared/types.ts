export interface Pair {
  id: string;
  key: string;
  value: string;
  enabled: boolean;
  secret?: boolean;
  local_value?: string | null;
}
export interface Auth {
  kind: "none" | "bearer" | "basic";
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
  id: string;
  name: string;
  method: string;
  url: string;
  description: string;
  query: Pair[];
  headers: Pair[];
  body_kind: "none" | "json" | "text" | "form";
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
