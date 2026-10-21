import type { WorkspaceData } from "../../shared/types";
import { LocalizedError } from "../../shared/i18n/errors";
import { collectionDescendants, removeCollectionTree } from "../collections/tree";

export type RemovalTarget = { kind: "collection" | "request"; id: string };
export type ResourceDependency = { kind: "scenario" | "dataset"; id: string; name: string };
function findRequest(data: WorkspaceData, requestId: string) {
  const collection = data.collections.find((item) => item.requests.some((request) => request.id === requestId));
  const request = collection?.requests.find((item) => item.id === requestId);
  if (!collection || !request) throw new LocalizedError("当前请求已不存在。");
  return { collection, request };
}
export function duplicateRequest(data: WorkspaceData, requestId: string, copyId: string, name: string): WorkspaceData {
  const { collection, request } = findRequest(data, requestId);
  if (!copyId || data.collections.some((item) => item.requests.some((item) => item.id === copyId))) throw new LocalizedError("请求 ID 已存在。");
  const copy = { ...structuredClone(request), id: copyId, name };
  return { ...data, collections: data.collections.map((item) => item.id === collection.id ? { ...item, requests: [...item.requests, copy] } : item) };
}
export function moveRequest(data: WorkspaceData, requestId: string, destinationId: string): WorkspaceData {
  const { collection, request } = findRequest(data, requestId);
  if (!data.collections.some((item) => item.id === destinationId)) throw new LocalizedError("找不到目录");
  if (collection.id === destinationId) return data;
  if (data.scenarios?.some((scenario) => scenario.steps.some((step) => step.request_id === requestId) && !collectionDescendants(data.collections, scenario.collection_id).has(destinationId))) throw new LocalizedError("移动后，请求仍须位于其场景的集合子树中。");
  return { ...data, collections: data.collections.map((item) => item.id === collection.id ? { ...item, requests: item.requests.filter((item) => item.id !== requestId) } : item.id === destinationId ? { ...item, requests: [...item.requests, request] } : item) };
}
export function removalImpact(data: WorkspaceData, target: RemovalTarget) {
  const collectionIds = target.kind === "collection" ? collectionDescendants(data.collections, target.id) : new Set<string>();
  if (target.kind === "collection" && !data.collections.some((item) => item.id === target.id)) throw new LocalizedError("找不到目录");
  if (target.kind === "request") findRequest(data, target.id);
  const requests = data.collections.flatMap((collection) => collection.requests.filter((request) => target.kind === "request" ? request.id === target.id : collectionIds.has(collection.id)));
  const requestIds = new Set(requests.map((request) => request.id));
  const dependencies: ResourceDependency[] = [
    ...(data.scenarios ?? []).filter((scenario) => collectionIds.has(scenario.collection_id) || scenario.steps.some((step) => requestIds.has(step.request_id))).map((scenario) => ({ kind: "scenario" as const, id: scenario.id, name: scenario.name })),
    ...(data.datasets ?? []).filter((dataset) => dataset.query && requestIds.has(dataset.query.request_id)).map((dataset) => ({ kind: "dataset" as const, id: dataset.id, name: dataset.name })),
  ];
  return { collectionCount: collectionIds.size, requestCount: requests.length, documentCount: data.documents?.filter((document) => document.collection_id && collectionIds.has(document.collection_id)).length ?? 0, dependencies };
}
export function removeResource(data: WorkspaceData, target: RemovalTarget): WorkspaceData {
  if (removalImpact(data, target).dependencies.length) throw new LocalizedError("所选资源仍被场景或数据集引用。请先更新引用，再删除。");
  if (target.kind === "collection") return removeCollectionTree(data, target.id);
  return { ...data, collections: data.collections.map((collection) => ({ ...collection, requests: collection.requests.filter((request) => request.id !== target.id) })) };
}
