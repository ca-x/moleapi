import type { RequestSpec, WorkspaceData } from "../../shared/types";
import { collectionAncestors } from "../collections/tree";
export function inheritedAuthSource(data: WorkspaceData, request: RequestSpec) {
  const collection=data.collections.find(c=>c.requests.some(r=>r.id===request.id));
  if (collection) {
    const path=collectionAncestors(data.collections,collection.id);
    const parent=path.reverse().find(c=>c.auth && c.auth.kind!=="inherit");
    if(parent)return {scope:parent.auth!.kind.includes("{{")?"dynamic" as const:"collection" as const,name:parent.name};
  }
  if(data.auth?.kind.includes("{{"))return {scope:"dynamic" as const,name:""};
  return {scope:data.auth && data.auth.kind!=="inherit"?"workspace" as const:"default" as const,name:""};
}
