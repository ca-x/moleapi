import { api, ApiError, native, previewSharedMock, token } from "./api";
import { message } from "./i18n";
import { expireNativeMemberSession, nativeMemberSession } from "./nativeMemberSession";
export function memberCredential(): string | null {
  if (!native) return token();
  const current = nativeMemberSession();
  return current.sessionId ? `native-member:${current.sessionId}:${current.epoch}` : null;
}
type Reply = { status: number; body: unknown; mock_preview: boolean };
function stale() { return new ApiError(message("托管连接已改变，请重新打开共享工作区。"), 409); }
function errorFrom(body: unknown, status: number) {
  let value = body;
  if (typeof value === "string") { try { value = JSON.parse(value); } catch { /* Raw failures use a bounded status message. */ } }
  const text = value && typeof value === "object" && "error" in value && typeof value.error === "string" ? value.error : message("托管成员请求失败 ({{value0}})", { value0: status });
  return new ApiError(text, status);
}
async function forward(path: string, method: string, body: unknown, preview = false): Promise<Reply> {
  const origin = nativeMemberSession();
  if (!origin.sessionId) throw stale();
  const { invoke } = await import("@tauri-apps/api/core");
  if (nativeMemberSession() !== origin) throw stale();
  const request = { session_id: origin.sessionId, method, path, ...(body === undefined ? {} : { body }), ...(preview ? { mode: "mock_preview" } : {}) };
  let result: { status: number; body: unknown };
  try { result = await invoke("api", { method: "POST", path: "/api/sync/collaboration", body: request }); }
  catch (error) { if (nativeMemberSession() !== origin) throw stale(); throw error; }
  if (nativeMemberSession() !== origin) throw stale();
  if (result.status < 200 || result.status >= 300) throw errorFrom(result.body, result.status);
  const reply = result.body as Reply;
  if (!reply || !Number.isInteger(reply.status) || reply.status < 100 || reply.status > 599 || typeof reply.mock_preview !== "boolean") throw new ApiError(message("托管成员响应格式无效。"), 502);
  if (reply.status === 401 && !reply.mock_preview) expireNativeMemberSession(origin.sessionId);
  return reply;
}
export async function memberApi<T>(path: string, ...args: [method?: string, body?: unknown]): Promise<T> {
  if (!native) return api<T>(path, ...args);
  const [method = "GET", body] = args;
  const reply = await forward(path, method, body);
  if (reply.status < 200 || reply.status >= 300) throw errorFrom(reply.body, reply.status);
  return reply.body as T;
}
export async function previewMemberMock(path: string): Promise<{ status: number; body: string }> {
  if (!native) return previewSharedMock(path);
  if (!/^\/api\/owners\/[^/?]+\/workspaces\/[^/?]+\/mock\/[^/?]+\/[^/?]+$/.test(path)) throw new ApiError(message("Mock 地址无效"), 400);
  const reply = await forward(path, "GET", undefined, true);
  if (!reply.mock_preview || typeof reply.body !== "string") throw errorFrom(reply.body, reply.status);
  return { status: reply.status, body: reply.body };
}

export async function closeMemberSession(path: string, credential: string | null, keepalive = false): Promise<void> {
  if (!credential || memberCredential() !== credential) return;
  if (native || !keepalive) { await memberApi(`${path}/close`, "POST", {}); return; }
  await fetch(`${path}/close`, { method: "POST", headers: { Authorization: `Bearer ${credential}` }, keepalive: true });
}
