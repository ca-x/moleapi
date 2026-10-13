// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Theme } from "@radix-ui/themes";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { setLanguage } from "../../shared/i18n";
import HostedRunnerManagement from "./HostedRunnerManagement";

const mocks = vi.hoisted(() => ({ api: vi.fn() }));
vi.mock("../../shared/api", () => ({ api: mocks.api, token: () => null }));
vi.mock("../../shared/ui", () => ({
  Field: ({ label, children }: { label: string; children: React.ReactNode }) => <label>{label}{children}</label>,
  Choice: ({ label, value, options, onChange, disabled }: { label: string; value: string; options: { value: string; label: string }[]; onChange: (value: string) => void; disabled: boolean }) =>
    <select aria-label={label} value={value} disabled={disabled} onChange={event => onChange(event.target.value)}>{options.map(option => <option key={option.value} value={option.value}>{option.label}</option>)}</select>,
}));
const runner = { id: "runner", name: "LAN", labels: ["lan"], enabled: true, revision: 1, created_at: "2026-10-13T00:00:00Z" };
const workspace = { id: "w", name: "Remote workspace", revision: 9, data: { collections: [{ id: "c", name: "Collection" }], scenarios: [], environments: [], datasets: [] } };
let client: QueryClient;
const view = (connectionId = "connection-1") => <QueryClientProvider client={client}><Theme><HostedRunnerManagement connectionId={connectionId} workspaceId="w" /></Theme></QueryClientProvider>;
beforeEach(async () => {
  await setLanguage("en");
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  mocks.api.mockReset().mockImplementation(async (path: string) => path === "/api/hosted/workspaces/w" ? workspace : path === "/api/hosted/runners" ? [runner] : []);
});
afterEach(() => { cleanup(); client.clear(); });

it("queues a label group using the hosted revision and leaves the offline source out of the payload", async () => {
  render(view()); fireEvent.click(screen.getByText("Hosted runner management"));
  await screen.findByText(/Hosted workspace revision 9/);
  fireEvent.change(screen.getByRole("combobox", { name: "Target runner" }), { target: { value: "label:lan" } });
  fireEvent.click(screen.getByRole("button", { name: "Dispatch to runner" }));
  await waitFor(() => expect(mocks.api).toHaveBeenCalledWith("/api/hosted/workspaces/w/runner-tasks", "POST", {
    runner_id: null, runner_label: "lan", expected_revision: 9, max_attempts: 1,
    selection: { collection_id: "c", scenario_id: null, environment_id: null, dataset_id: null, iterations: null, notification_ids: [] },
  }));
});

it("saves hosted schedules with a label target and canonical hosted collection", async () => {
  render(view()); fireEvent.click(screen.getByText("Hosted runner management"));
  await screen.findByText(/Hosted workspace revision 9/);
  fireEvent.change(screen.getByRole("combobox", { name: "Scheduled execution target" }), { target: { value: "label:lan" } });
  fireEvent.change(screen.getByRole("textbox", { name: "Task name" }), { target: { value: "LAN checks" } });
  fireEvent.click(screen.getByRole("button", { name: "Save scheduled task" }));
  await waitFor(() => expect(mocks.api).toHaveBeenCalledWith("/api/hosted/workspaces/w/schedules", "POST", expect.objectContaining({
    runner_id: null, runner_label: "lan", collection_id: "c", name: "LAN checks",
  })));
});

it("drops a late one-time credential after the connection changes", async () => {
  let resolve!: (value: unknown) => void;
  mocks.api.mockImplementation((path: string, method?: string) => path === "/api/hosted/workspaces/w" ? Promise.resolve(workspace) : path === "/api/hosted/runners" ? Promise.resolve([runner]) : path === "/api/hosted/runners/runner/credentials" && method === "POST" ? new Promise(done => { resolve = done; }) : Promise.resolve([]));
  const rendered = render(view()); fireEvent.click(screen.getByText("Hosted runner management"));
  fireEvent.click(await screen.findByRole("button", { name: "Manage credentials" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Credential name" }), { target: { value: "Agent" } });
  fireEvent.click(screen.getByRole("button", { name: "Create runner credential" }));
  await waitFor(() => expect(mocks.api).toHaveBeenCalledWith("/api/hosted/runners/runner/credentials", "POST", expect.anything()));
  rendered.rerender(view("connection-2"));
  await act(async () => resolve({ token: "private-old-connection-token" }));
  expect(document.body.textContent).not.toContain("private-old-connection-token");
  expect(screen.queryByLabelText("New runner credential")).toBeNull();
});

it("opens owned hosted report and response details through the limited gateway", async () => {
  const report = { id: "report", summary: { passed: 1, failed: 0 }, results: [{ position: 0, request_name: "Check", status: 200, history_id: "history" }] };
  const response = { id: "history", response: { status: 200, status_text: "OK", body: "redacted response" } };
  mocks.api.mockImplementation(async (path: string) => path === "/api/hosted/workspaces/w" ? workspace : path === "/api/hosted/runners" ? [runner] : path.endsWith("/runner-tasks") ? [{ id: "task", runner_id: "runner", status: "completed", report_id: "report" }] : path.endsWith("/reports/report") ? report : path.endsWith("/steps/0/response") ? response : []);
  render(view()); fireEvent.click(screen.getByText("Hosted runner management"));
  fireEvent.click(await screen.findByRole("button", { name: "View task report" }));
  fireEvent.click(await screen.findByRole("button", { name: "View response" }));
  expect(await screen.findByLabelText("Hosted response preview")).toHaveProperty("value", "200 OK\nredacted response");
});
