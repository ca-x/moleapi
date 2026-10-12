export interface ExecutionRunner {
  id: string;
  name: string;
  enabled: boolean;
  revision: number;
  last_seen_at: string | null;
  active_task_id: string | null;
  created_at: string;
}
export interface RunnerSelection {
  collection_id: string;
  scenario_id: string | null;
  environment_id: string | null;
  dataset_id: string | null;
  iterations: number | null;
  notification_ids: string[];
}
export interface RunnerTask {
  id: string;
  workspace_id: string;
  runner_id: string;
  selection: RunnerSelection;
  source_revision: number;
  status: "queued" | "leased" | "completed" | "failed" | "cancelled";
  attempt: number;
  max_attempts: number;
  queued_at: string;
  started_at: string | null;
  lease_until: string | null;
  finished_at: string | null;
  report_id: string | null;
}
