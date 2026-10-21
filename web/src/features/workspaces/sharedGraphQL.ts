import { getOperationAST, parse } from "graphql";
import stable from "fast-json-stable-stringify";
import type { RequestSpec } from "../../shared/types";
export function savedSubscriptionMatches(saved: RequestSpec, selected: RequestSpec) {
  const source = saved.protocol, candidate = selected.protocol;
  if (source?.kind !== "graphql" || candidate?.kind !== "graphql" || source.document !== candidate.document) return false;
  try {
    const original = getOperationAST(parse(source.document), source.operation_name);
    const requested = getOperationAST(parse(candidate.document), candidate.operation_name);
    const variables = source.variables_source == null ? source.variables : JSON.parse(source.variables_source || "{}");
    return original?.operation === "subscription" && requested?.operation === "subscription" && original.name?.value === requested.name?.value && stable(variables) === stable(candidate.variables);
  } catch { return false; }
}
