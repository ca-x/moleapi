import type { SyncStatus } from "./types";
export type NativeMemberSnapshot = { sessionId: string | null; epoch: number };
let snapshot: NativeMemberSnapshot = { sessionId: null, epoch: 0 };
const listeners = new Set<() => void>();
const expired = new Set<string>();
export const nativeMemberSession = () => snapshot;
export function subscribeNativeMember(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener); }; }
function replace(sessionId: string | null) { snapshot = { sessionId, epoch: snapshot.epoch + 1 }; for (const listener of listeners) listener(); }
export function beginNativeMemberChange() { replace(null); return snapshot.epoch; }
export function applyNativeMemberStatus(status: SyncStatus, expectedEpoch: number): boolean {
  if (snapshot.epoch !== expectedEpoch) return false;
  const sessionId = status.connected && status.session_id ? status.session_id : null;
  if (sessionId && expired.has(sessionId)) return false;
  if (snapshot.sessionId !== sessionId) replace(sessionId);
  return true;
}
export function expireNativeMemberSession(sessionId: string): boolean {
  if (snapshot.sessionId !== sessionId) return false;
  expired.add(sessionId);
  if (expired.size > 64) expired.delete(expired.values().next().value!);
  replace(null);
  if (typeof window !== "undefined") window.dispatchEvent(new Event("moleapi:hosted-unauthorized"));
  return true;
}
