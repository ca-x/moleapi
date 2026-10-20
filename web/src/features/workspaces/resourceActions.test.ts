import { expect, it } from "vitest";
import { initialData } from "../../shared/model";
import { duplicateRequest, moveRequest, removalImpact, removeResource } from "./resourceActions";
function fixture() {
  const data = initialData(); data.collections[0].id = "root";
  const request = data.collections[0].requests[0]; request.id = "request";
  data.collections.push({ id: "child", parent_id: "root", name: "Child", description: "", requests: [] }, { id: "other", name: "Other", description: "", requests: [] });
  data.documents = [{ id: "doc", name: "Guide", source: "keep authored source", collection_id: "child" }];
  return data;
}
it("duplicates and moves requests without altering the original source", () => {
  const data = fixture(); const original = structuredClone(data.collections[0].requests[0]);
  const copied = duplicateRequest(data, "request", "copy-id", "Copy");
  expect(copied.collections[0].requests.at(-1)).toEqual({ ...original, id: "copy-id", name: "Copy" });
  expect(data.collections[0].requests).toHaveLength(2);
  const moved = moveRequest(copied, "copy-id", "child");
  expect(moved.collections[1].requests[0]).toEqual({ ...original, id: "copy-id", name: "Copy" });
  expect(moved.collections[0].requests).toEqual(data.collections[0].requests);
});
it("removes a collection subtree and reparents documents without touching unrelated resources", () => {
  const data = fixture(); const target = { kind: "collection" as const, id: "root" };
  expect(removalImpact(data, target)).toMatchObject({ collectionCount: 2, requestCount: 2, documentCount: 1, dependencies: [] });
  const removed = removeResource(data, target);
  expect(removed.collections.map((collection) => collection.id)).toEqual(["other"]);
  expect(removed.documents?.[0]).toMatchObject({ source: "keep authored source", collection_id: null });
});
it("keeps referencing scenarios and datasets intact and rejects dangling deletion or moves", () => {
  const data = fixture();
  data.scenarios = [{ id: "scenario", name: "Suite", description: "", collection_id: "root", steps: [{ id: "step", request_id: "request", name: "Step", group: "", enabled: true }] }];
  data.datasets = [{ id: "dataset", name: "Rows", description: "", secret: true, query: { request_id: "request" } }];
  const target = { kind: "request" as const, id: "request" };
  expect(removalImpact(data, target).dependencies).toEqual([{ kind: "scenario", id: "scenario", name: "Suite" }, { kind: "dataset", id: "dataset", name: "Rows" }]);
  expect(() => removeResource(data, target)).toThrow();
  expect(() => moveRequest(data, "request", "other")).toThrow();
  expect(moveRequest(data, "request", "child").scenarios).toEqual(data.scenarios);
  expect(data.collections[0].requests[0].id).toBe("request");
});
