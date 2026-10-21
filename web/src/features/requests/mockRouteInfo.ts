import type { RequestSpec } from "../../shared/types";

export function mockRouteInfo(request: RequestSpec): { path: string; hasSavedQuery: boolean } {
  let hasSavedQuery = request.query.some((row) => row.enabled);
  if (request.protocol?.kind && request.protocol.kind !== "http") return { path: "", hasSavedQuery };
  try {
    const url = new URL(request.url);
    hasSavedQuery ||= url.search !== "";
    if (!["http:", "https:"].includes(url.protocol) || /[{}]/.test(url.host)) return { path: "", hasSavedQuery };
    const path = url.pathname.replace(/%7B/gi, "{").replace(/%7D/gi, "}");
    if (path.split("/").some((segment) => /[{}]/.test(segment) && !/^[{][A-Za-z_][A-Za-z0-9_]{0,63}[}]$/.test(segment))) return { path: "", hasSavedQuery };
    return { path, hasSavedQuery };
  } catch { return { path: "", hasSavedQuery }; }
}
