import { getOperationAST, parse } from "graphql";
import type { RequestSpec } from "../../shared/types";
export function authenticationEligibility(request: RequestSpec) {
  const protocol = request.protocol;
  const kind = protocol?.kind ?? "http";
  const api = !["tcp", "mqtt"].includes(kind)
    && !(protocol?.kind === "mcp" && protocol.transport === "stdio")
    && !(protocol?.kind === "data" && protocol.source !== "remote_file");
  let digest = ["http", "soap"].includes(kind);
  if (protocol?.kind === "graphql") {
    try {
      const operation = getOperationAST(parse(protocol.document), protocol.operation_name);
      digest = !!operation && operation.operation !== "subscription";
    } catch { digest = false; }
  }
  return { api, digest, query: kind !== "grpc" };
}
