import type { SavedDatasetView, WorkspaceData } from "../../shared/types";
export function datasetBindings(data: WorkspaceData, datasetId: string, replacementViews?: SavedDatasetView[]) {
  return data.collections.flatMap((collection) => collection.requests.flatMap((request) => request.examples.flatMap((example) => {
    const binding = example.mock_data;
    if (!binding || binding.dataset_id !== datasetId || (replacementViews && replacementViews.some((view) => view.id === binding.view_id && !!view.sql?.trim()))) return [];
    return [{ requestId: request.id, requestName: request.name, exampleId: example.id, exampleName: example.name, viewId: binding.view_id }];
  })));
}
