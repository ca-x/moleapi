import type { Pair } from "../../shared/types";
import { pair } from "../../shared/model";
export interface Receiver {
  id: string; workspace_id: string; name: string; token: string; active: boolean;
  receiver_path: string; revision: number; config_epoch: number; received: number; dropped: number;
  response: { status: number; headers: Pair[]; body: string };
}
export interface Capture {
  id: string; cursor: number; method: string; query: string; received_at: string;
  headers: { name: string; value_text: string | null; value_base64: string }[];
  body_base64: string; body_text: string | null; body_bytes: number; response_status: number; redacted: boolean;
}
export interface Batch { captures: Capture[]; received: number; dropped: number; revision: number }
export interface Listener { active: boolean; origin: string | null; bind: string | null }
export function replayRows(capture: Capture): { headers: Pair[]; query: Pair[] } {
  return {
    headers: capture.headers.filter(h => h.value_text !== null).map(h => ({ ...pair(), key: h.name, value: h.value_text! })),
    query: Array.from(new URLSearchParams(capture.query), ([key, value]) => ({ ...pair(), key, value })),
  };
}
