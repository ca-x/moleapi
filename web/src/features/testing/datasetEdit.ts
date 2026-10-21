import { id } from "../../shared/model";
import type { SavedDataset, SavedDatasetView } from "../../shared/types";
export interface DatasetEdit { id: string; name: string; table_name: string; views: SavedDatasetView[]; description: string; secret: boolean; format: "csv" | "json"; source: string; mode: "literal" | "query"; requestId: string }
export function datasetEdit(dataset?: SavedDataset): DatasetEdit {
  return { id: dataset?.id ?? "", name: dataset?.name ?? "", table_name: dataset?.table_name ?? "data", views: dataset?.views ?? [], description: dataset?.description ?? "", secret: dataset?.secret ?? true, format: dataset?.source?.format ?? "json", source: dataset?.source?.source ?? "", mode: dataset?.query ? "query" : "literal", requestId: dataset?.query?.request_id ?? "" };
}
export function validDatasetEdit(edit: DatasetEdit, datasets: SavedDataset[], requests: { value: string }[]) {
  const bytes = (value: string) => new TextEncoder().encode(value).length;
  return !!edit.name.trim() && bytes(edit.name) <= 256 && bytes(edit.description) <= 4096 && /^[A-Za-z_][A-Za-z0-9_]{0,63}$/.test(edit.table_name)
    && (!!edit.id || datasets.length < 20) && edit.views.length <= 20 && new Set(edit.views.map((view) => view.id)).size === edit.views.length
    && edit.views.every((view) => !!view.name.trim() && bytes(view.name) <= 256 && (view.sql === undefined || (!!view.sql.trim() && bytes(view.sql) <= 65536)))
    && edit.views.reduce((size, view) => size + bytes(view.sql ?? ""), 0) <= 1024 * 1024
    && (edit.mode === "query" ? requests.some((request) => request.value === edit.requestId) : bytes(edit.source) <= 1024 * 1024)
    && datasets.filter((dataset) => dataset.id !== edit.id).reduce((size, dataset) => size + bytes(dataset.source?.source ?? ""), edit.mode === "literal" ? bytes(edit.source) : 0) <= 4 * 1024 * 1024;
}
export function datasetFromEdit(edit: DatasetEdit): SavedDataset {
  return { id: edit.id || id(), name: edit.name.trim(), table_name: edit.table_name, views: edit.views, description: edit.description, secret: edit.mode === "query" ? true : edit.secret,
    ...(edit.mode === "query" ? { query: { request_id: edit.requestId } } : edit.source.trim() ? { source: { format: edit.format, source: edit.source } } : {}) };
}
