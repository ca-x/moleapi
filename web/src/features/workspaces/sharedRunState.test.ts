import { expect, it } from "vitest";
import type { SavedDataset } from "../../shared/types";
import { defaultSharedRunConfig, resolveSharedRunOptions } from "./sharedRunState";
const datasets: SavedDataset[] = [
  { id: "literal", name: "Literal", description: "", secret: true, source: { format: "json", source: '[{"id":1}]' } },
  { id: "query", name: "Query", description: "", secret: true, query: { request_id: "sql" } },
  { id: "empty", name: "Empty", description: "", secret: true },
];
it("defaults to no data and never copies saved private source", () => {
  expect(resolveSharedRunOptions(defaultSharedRunConfig(), datasets)).toEqual({ options: {}, problem: null });
  for (const id of ["literal", "query"]) expect(resolveSharedRunOptions({ ...defaultSharedRunConfig(), selection: `dataset:${id}`, iterations: "2", source: "ignored" }, datasets)).toEqual({ options: { dataset_id: id, iterations: 2 }, problem: null });
});
it.each(["dataset:deleted", "dataset:empty", "invalid"])("blocks unavailable selection %s", (selection) => {
  expect(resolveSharedRunOptions({ ...defaultSharedRunConfig(), selection }, datasets)).toEqual({ options: null, problem: "dataset" });
});
it.each(["0", "101", "1.5", "NaN"])("blocks invalid iterations %s", (iterations) => {
  expect(resolveSharedRunOptions({ ...defaultSharedRunConfig(), iterations }, datasets)).toEqual({ options: null, problem: "input" });
});
it("requires explicit temporary source and enforces byte limit", () => {
  const config = { ...defaultSharedRunConfig(), selection: "temporary" };
  expect(resolveSharedRunOptions(config, datasets).problem).toBe("input");
  expect(resolveSharedRunOptions({ ...config, source: "汉".repeat(400000) }, datasets).problem).toBe("input");
  expect(resolveSharedRunOptions({ ...config, format: "csv", source: "id\n1", iterations: "3" }, datasets)).toEqual({ options: { dataset: { format: "csv", source: "id\n1" }, iterations: 3 }, problem: null });
});
