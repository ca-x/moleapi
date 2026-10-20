import type { WorkspaceData } from "../../shared/types";
export function specificationReferences(data: WorkspaceData, specificationId: string) {
  return data.collections.flatMap((collection) => collection.requests.flatMap((request) => [
    ...(request.specification_id === specificationId ? [{ requestId: request.id, requestName: request.name, exampleName: "" }] : []),
    ...request.examples.filter((example) => example.mock_schema_link?.specification_id === specificationId).map((example) => ({ requestId: request.id, requestName: request.name, exampleName: example.name })),
  ]));
}
