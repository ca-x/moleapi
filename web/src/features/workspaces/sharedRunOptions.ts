import type { RunOptions, SavedDataset } from "../../shared/types";
import { runnerOptions, type RunnerConfig } from "../testing/RunnerDataset";
export interface SharedRunConfig extends RunnerConfig { selection: string }
export const defaultSharedRunConfig = (): SharedRunConfig => ({ selection: "none", format: "json", source: "", iterations: "" });
export function resolveSharedRunOptions(config: SharedRunConfig, datasets: SavedDataset[]): { options: RunOptions | null; problem: "input" | "dataset" | null } {
  try {
    const options = runnerOptions({ ...config, source: config.selection === "temporary" ? config.source : "" });
    if (config.selection === "none") return { options, problem: null };
    if (config.selection === "temporary") return options.dataset ? { options, problem: null } : { options: null, problem: "input" };
    const dataset = config.selection.startsWith("dataset:") ? datasets.find((item) => `dataset:${item.id}` === config.selection) : undefined;
    if (!dataset || (!dataset.query?.request_id && !dataset.source?.source.trim())) return { options: null, problem: "dataset" };
    return { options: { ...options, dataset_id: dataset.id }, problem: null };
  } catch { return { options: null, problem: "input" }; }
}
