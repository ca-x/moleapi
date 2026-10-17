import { parse } from "yaml";

export interface OpenapiMockChoice {
  pointer: string;
  label: string;
  status: number;
  mediaType: string;
}

const object = (value: unknown): Record<string, unknown> | null => value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : null;
const escapePointer = (value: string) => value.replace(/~/g,"~0").replace(/\//g,"~1");
const unescapePointer = (value: string) => value.replace(/~1/g,"/").replace(/~0/g,"~");

export function openapiMockChoices(source: string, requestMethod: string): OpenapiMockChoice[] {
  if (new TextEncoder().encode(source).length > 5 * 1024 * 1024) return [];
  let document: Record<string, unknown> | null;
  try { document = object(parse(source)); } catch { return []; }
  if (!document || !/^3\.[01]\./.test(String(document.openapi ?? ""))) return [];
  const method = requestMethod.toLowerCase();
  const paths = object(document.paths);
  if (!paths) return [];
  const result: OpenapiMockChoice[] = [];
  for (const [path, rawItem] of Object.entries(paths)) {
    const operation = object(object(rawItem)?.[method]);
    const responses = object(operation?.responses);
    if (!responses) continue;
    for (const [statusText, rawResponse] of Object.entries(responses)) {
      const status = Number(statusText);
      if (!Number.isInteger(status) || status < 200 || status > 599 || String(status) !== statusText) continue;
      let response = object(rawResponse);
      const seen = new Set<string>();
      for (let depth=0; depth<64 && response?.$ref; depth++) {
        const reference = response.$ref;
        if (typeof reference !== "string" || !reference.startsWith("#/components/responses/") || seen.has(reference)) { response=null; break; }
        seen.add(reference);
        const name = unescapePointer(reference.slice("#/components/responses/".length));
        response = object(object(object(document.components)?.responses)?.[name]);
      }
      const content = object(response?.content);
      if (!content) continue;
      for (const [mediaType, media] of Object.entries(content)) {
        const mime = mediaType.split(";")[0].trim().toLowerCase();
        if (mime !== "application/json" && !mime.endsWith("+json")) continue;
        if (!object(media)?.schema) continue;
        result.push({
          pointer: `#/paths/${escapePointer(path)}/${method}/responses/${statusText}/content/${escapePointer(mediaType)}/schema`,
          label: `${method.toUpperCase()} ${path} · ${statusText} ${mediaType}`,
          status,
          mediaType,
        });
      }
    }
  }
  return result.sort((left,right)=>left.label.localeCompare(right.label));
}
