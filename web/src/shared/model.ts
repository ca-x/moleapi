import { t } from "./i18n";
import stableStringify from "fast-json-stable-stringify";
import type { Pair, RequestSpec, WorkspaceData } from "./types";
export const id = () => crypto.randomUUID();
export const pair = (key = "", value = "", secret = false): Pair => ({
  id: id(),
  key,
  value,
  enabled: true,
  secret,
});
export function newRequest(name = t("新建请求"), url = ""): RequestSpec {
  return {
    id: id(),
    name,
    method: "GET",
    url,
    description: "",
    query: [],
    headers: [],
    body_kind: "none",
    body: "",
    auth: { kind: "none", token: "", username: "", password: "" },
    timeout_ms: 30000,
    follow_redirects: true,
    verify_tls: true,
    assertions: [],
    examples: [],
  };
}
export function initialData(): WorkspaceData {
  const health = newRequest(t("发送 Echo 请求"), "{{base_url}}/get");
  health.description =
    t("使用 Apifox 的公开 Echo 服务查看请求内容，不需要鉴权。\n\n点击「发送」即可查看返回状态、耗时和 JSON 内容。");
  health.assertions = [
    {
      id: id(),
      name: t("HTTP 状态为 200"),
      kind: "status",
      target: "",
      expected: "200",
    },
  ];
  const create = newRequest(t("发送 Echo POST 请求"), "{{base_url}}/post");
  create.method = "POST";
  create.body_kind = "json";
  create.body = '{\n  "name": "MoleAPI"\n}';
  create.description =
    t("向 Apifox 公开 Echo 服务发送 JSON，响应会返回你提交的请求内容。\n\n点击「请求体」修改 JSON，再点击「发送」查看结果。不需要鉴权。");
  create.assertions = [
    {
      id: id(),
      name: t("HTTP 状态为 200"),
      kind: "status",
      target: "",
      expected: "200",
    },
    {
      id: id(),
      name: t("Echo 返回提交的名称"),
      kind: "json",
      target: "/json/name",
      expected: '"MoleAPI"',
    },
  ];
  return {
    schema_version: 1,
    collections: [
      {
        id: id(),
        name: t("快速开始"),
        description: t("了解工作台的基本操作"),
        requests: [health, create],
      },
    ],
    environments: [
      {
        id: "local",
        name: t("本地开发"),
        variables: [
          pair("base_url", nativeBase()),
          pair("api_token", "", true),
        ],
      },
    ],
    active_environment_id: "local",
  };
}
function nativeBase() {
  return "https://echo.apifox.com";
}
export const fingerprint = (data: { name: string; data: WorkspaceData }) =>
  stableStringify({ name: data.name, data: data.data });
export const bytes = (value: number) =>
  value < 1024
    ? `${value} B`
    : value < 1024 * 1024
      ? `${(value / 1024).toFixed(1)} KB`
      : `${(value / (1024 * 1024)).toFixed(1)} MB`;
const quote = (value: string) => `'${value.replaceAll("'", "'\\''")}'`;
const sensitiveKey =
  /^(?:authorization|proxy-authorization|cookie|set-cookie|x-api-key|api[-_]?key|api[-_]?token|access[-_]?token|refresh[-_]?token|token|password|secret|client[-_]?secret)$/i;
function templateUrl(request: RequestSpec): string {
  const [beforeHash, ...fragment] = request.url.split("#");
  const [base, ...existingQuery] = beforeHash.split("?");
  const query = existingQuery
    .join("?")
    .split("&")
    .filter(Boolean)
    .map((part) => {
      const key = part.split("=", 1)[0];
      let decoded = key;
      try {
        decoded = decodeURIComponent(key);
      } catch {
        /* Keep malformed URL text editable. */
      }
      return sensitiveKey.test(decoded) ? `${key}={{REDACTED}}` : part;
    });
  query.push(
    ...request.query
      .filter((row) => row.enabled && row.key)
      .map(
        (row) =>
          `${encodeURIComponent(row.key)}=${row.secret || sensitiveKey.test(row.key) ? "{{REDACTED}}" : encodeURIComponent(row.value)}`,
      ),
  );
  const safeBase = base.replace(
    /^(https?:\/\/)[^/@]+:[^/@]+@/i,
    "$1{{USERNAME}}:{{PASSWORD}}@",
  );
  return `${safeBase}${query.length ? "?" + query.join("&") : ""}${fragment.length ? "#" + fragment.join("#") : ""}`;
}
export function curlTemplate(request: RequestSpec): string {
  const parts = ["curl", "-X", request.method, quote(templateUrl(request))];
  for (const row of request.headers.filter((x) => x.enabled && x.key))
    parts.push(
      "-H",
      quote(
        `${row.key}: ${row.secret || sensitiveKey.test(row.key) ? "{{REDACTED}}" : row.value}`,
      ),
    );
  if (request.auth.kind === "bearer")
    parts.push("-H", quote("Authorization: Bearer {{TOKEN}}"));
  if (request.auth.kind === "basic")
    parts.push("-u", quote("{{USERNAME}}:{{PASSWORD}}"));
  if (["binary","multipart"].includes(request.body_kind)) throw new Error("File-body cURL templates are not available; export MoleAPI source instead");
  if (request.body_kind !== "none") {
    if (request.body_kind === "json")
      parts.push("-H", quote("Content-Type: application/json"));
    if (request.body_kind === "form")
      parts.push(
        "-H",
        quote("Content-Type: application/x-www-form-urlencoded"),
      );
    parts.push("--data-raw", quote(request.body));
  }
  if (request.follow_redirects) parts.push("-L");
  if (!request.verify_tls) parts.push("-k");
  parts.push("--max-time", String(request.timeout_ms / 1000));
  return parts.join(" ");
}
export function safeMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export const requestLabel = (request: RequestSpec) =>
  request.protocol?.kind === "data" ? "Data" : request.protocol?.kind === "tcp" ? "TCP" : request.protocol?.kind === "a2a" ? "A2A" : request.protocol?.kind === "mcp" ? "MCP" : request.protocol?.kind === "soap"
    ? "SOAP"
    : request.protocol?.kind === "mqtt"
      ? "MQTT"
      : request.protocol?.kind === "socketio"
        ? "IO"
        : request.protocol?.kind === "grpc"
          ? "gRPC"
          : request.protocol?.kind === "graphql"
            ? "GQL"
            : request.protocol?.kind === "websocket"
              ? "WS"
              : request.protocol?.kind === "sse"
                ? "SSE"
                : request.method;
