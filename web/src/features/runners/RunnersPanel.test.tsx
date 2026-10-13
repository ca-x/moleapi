// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Theme } from "@radix-ui/themes";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { setLanguage } from "../../shared/i18n";
import RunnersPanel from "./RunnersPanel";
const mocks = vi.hoisted(() => ({ api: vi.fn(), native: false, credential: "first", save: vi.fn(), state: { accountId: "owner", authenticated: true, dirty: true, draft: { id: "w", revision: 1, data: { collections: [{ id: "c", name: "Collection" }], environments: [{ id: "dev", name: "Development" }], scenarios: [{ id: "s", name: "Scenario", collection_id: "c" }], datasets: [{ id: "data", name: "Data", source: { format: "json", source: "[]" } }] } } } }));
vi.mock("../../shared/api", () => ({ api: mocks.api, get native() { return mocks.native; }, token: () => mocks.credential }));
vi.mock("../workbench/context", () => ({ useWorkbench: () => ({ ...mocks.state, save: mocks.save }) }));
vi.mock("../notifications/NotificationPicker", () => ({ default: () => null }));
vi.mock("../../shared/ui", () => ({ Field: ({ label, children }: { label: string; children: React.ReactNode }) => <label>{label}{children}</label>, Choice: ({ label, value, options, onChange, disabled }: { label: string; value: string; options: { value: string; label: string }[]; onChange: (value: string) => void; disabled: boolean }) => <select aria-label={label} value={value} disabled={disabled} onChange={event => onChange(event.target.value)}>{options.map(option => <option key={option.value} value={option.value}>{option.label}</option>)}</select> }));
const runner = { id: "runner", name: "LAN", labels: ["linux", "lan"], enabled: true, revision: 4, last_seen_at: null, active_task_id: null, created_at: "2026-10-11T00:00:00Z" };
let client: QueryClient;
function view(onReport = vi.fn()) { return <QueryClientProvider client={client}><Theme><RunnersPanel onReport={onReport} /></Theme></QueryClientProvider>; }
beforeEach(async () => { await setLanguage("en"); mocks.native = false; mocks.credential = "first"; mocks.state.accountId = "owner"; mocks.state.draft.id = "w"; mocks.state.dirty = true; mocks.api.mockReset(); mocks.save.mockReset().mockResolvedValue({ ...mocks.state.draft, revision: 7 }); mocks.api.mockImplementation(async path => path === "/api/runners" ? [runner] : []); client = new QueryClient({ defaultOptions: { queries: { retry: false } } }); });
afterEach(() => { cleanup(); client.clear(); });
it("saves dirty sources before queueing the canonical revision and selected references without private overrides", async () => {
  render(view()); fireEvent.click(screen.getByText("Self-hosted runners")); await screen.findByRole("button", { name: "Edit runner" });
  fireEvent.change(screen.getByRole("combobox", { name: "Target runner" }), { target: { value: "runner" } });
  fireEvent.change(screen.getByRole("combobox", { name: "Task scenario" }), { target: { value: "s" } });
  fireEvent.change(screen.getByRole("combobox", { name: "Task environment" }), { target: { value: "dev" } });
  fireEvent.change(screen.getByRole("combobox", { name: "Task dataset" }), { target: { value: "data" } });
  fireEvent.click(screen.getByRole("button", { name: "Dispatch to runner" }));
  await waitFor(() => expect(mocks.api).toHaveBeenCalledWith("/api/workspaces/w/runner-tasks", "POST", { runner_id: "runner", expected_revision: 7, max_attempts: 1, selection: { collection_id: "c", scenario_id: "s", environment_id: "dev", dataset_id: "data", iterations: null, notification_ids: [] } }));
  expect(mocks.save).toHaveBeenCalledWith(true);
});
it("never dispatches after a failed save or a credential change while saving", async () => {
  mocks.save.mockResolvedValueOnce(null); const rendered = render(view()); fireEvent.click(screen.getByText("Self-hosted runners")); await screen.findByRole("button", { name: "Edit runner" });
  fireEvent.change(screen.getByRole("combobox", { name: "Target runner" }), { target: { value: "runner" } }); fireEvent.click(screen.getByRole("button", { name: "Dispatch to runner" }));
  await waitFor(() => expect(mocks.save).toHaveBeenCalledTimes(1)); await waitFor(() => expect(screen.getByRole("button", { name: "Dispatch to runner" })).toHaveProperty("disabled", false));
  expect(mocks.api).not.toHaveBeenCalledWith("/api/workspaces/w/runner-tasks", "POST", expect.anything());
  let resolve!: (value: unknown) => void; mocks.save.mockImplementationOnce(() => new Promise(done => { resolve = done; })); fireEvent.click(screen.getByRole("button", { name: "Dispatch to runner" })); await waitFor(() => expect(mocks.save).toHaveBeenCalledTimes(2));
  mocks.credential = "replacement"; rendered.rerender(view()); await act(async () => resolve({ ...mocks.state.draft, revision: 8 }));
  expect(mocks.api).not.toHaveBeenCalledWith("/api/workspaces/w/runner-tasks", "POST", expect.anything());
});
it("requires confirmation to disable and uses registry revisions for rename and enablement", async () => {
  render(view()); fireEvent.click(screen.getByText("Self-hosted runners")); await screen.findByRole("button", { name: "Edit runner" });
  expect(screen.getByText("#linux")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Edit runner" })); fireEvent.change(screen.getByLabelText("New runner name"), { target: { value: "Changed" } }); fireEvent.change(screen.getAllByLabelText("Runner labels (comma-separated)")[1], { target: { value: "LAN, ci" } }); fireEvent.click(screen.getByRole("button", { name: "Save runner" }));
  await waitFor(() => expect(mocks.api).toHaveBeenCalledWith("/api/runners/runner", "PATCH", { name: "Changed", enabled: true, expected_revision: 4, labels: ["lan", "ci"] }));
  await waitFor(() => expect(screen.getByRole("button", { name: "Disable runner" })).toHaveProperty("disabled", false)); fireEvent.click(screen.getByRole("button", { name: "Disable runner" }));
  expect(mocks.api).not.toHaveBeenCalledWith("/api/runners/runner", "PATCH", expect.objectContaining({ enabled: false })); fireEvent.click(screen.getByRole("button", { name: "Confirm disable" }));
  await waitFor(() => expect(mocks.api).toHaveBeenCalledWith("/api/runners/runner", "PATCH", { name: "LAN", enabled: false, expected_revision: 4 }));
});
it("filters dispatch targets by label without silently changing the selected runner", async () => {
  render(view()); fireEvent.click(screen.getByText("Self-hosted runners")); await screen.findByRole("button", { name: "Edit runner" });
  fireEvent.change(screen.getByRole("combobox", { name: "Target runner" }), { target: { value: "runner" } });
  fireEvent.change(screen.getByRole("combobox", { name: "Filter by label" }), { target: { value: "lan" } });
  expect(screen.getByRole("combobox", { name: "Target runner" })).toHaveProperty("value", "none");
  expect(screen.getByRole("button", { name: "Dispatch to runner" })).toHaveProperty("disabled", true);
  fireEvent.change(screen.getByRole("combobox", { name: "Target runner" }), { target: { value: "runner" } });
  expect(screen.getByRole("button", { name: "Dispatch to runner" })).toHaveProperty("disabled", false);
});
it("cancels owned tasks, opens reports and does not invoke hosted APIs in native mode", async () => {
  const onReport = vi.fn(); mocks.api.mockImplementation(async path => path === "/api/runners" ? [runner] : path.endsWith("/runner-tasks") ? [{ id: "queued", runner_id: "runner", source_revision: 1, status: "queued", queued_at: runner.created_at, attempt: 0, max_attempts: 1 }, { id: "done", runner_id: "runner", source_revision: 1, status: "completed", queued_at: runner.created_at, attempt: 1, max_attempts: 1, report_id: "report" }] : []);
  const rendered = render(view(onReport)); fireEvent.click(screen.getByText("Self-hosted runners")); fireEvent.click(await screen.findByRole("button", { name: "Cancel task execution" }));
  await waitFor(() => expect(mocks.api).toHaveBeenCalledWith("/api/workspaces/w/runner-tasks/queued/cancel", "POST")); fireEvent.click(screen.getByRole("button", { name: "View task report" })); expect(onReport).toHaveBeenCalledWith("report");
  rendered.unmount(); mocks.native = true; mocks.api.mockClear(); render(view()); expect(screen.getByText(/Manage runners on the hosted/)).toBeTruthy(); expect(mocks.api).not.toHaveBeenCalled();
});
it("discards late mutation errors after switching accounts", async () => {
  let reject!: (value: Error) => void; mocks.api.mockImplementation((path, method) => method === "POST" ? new Promise((_resolve, fail) => { reject = fail; }) : Promise.resolve(path === "/api/runners" ? [runner] : []));
  const rendered = render(view()); fireEvent.click(screen.getByText("Self-hosted runners")); fireEvent.change(screen.getByLabelText("Runner name"), { target: { value: "Private runner" } }); fireEvent.change(screen.getByLabelText("Runner labels (comma-separated)"), { target: { value: "CI, lan" } }); fireEvent.click(screen.getByRole("button", { name: "Register runner" })); await waitFor(() => expect(mocks.api).toHaveBeenCalledWith("/api/runners", "POST", { name: "Private runner", labels: ["ci", "lan"] }));
  mocks.state.accountId = "other"; mocks.credential = "other-credential"; rendered.rerender(view()); await act(async () => reject(new Error("old-private-error"))); expect(document.body.textContent).not.toContain("old-private-error");
});
it("issues a workspace credential once and confirms revocation without retaining the secret", async () => {
  const credential = { id: "cred", runner_id: "runner", workspace_id: "w", name: "Agent", created_at: 1, expires_at: 9999999999, expired: false };
  mocks.api.mockImplementation(async (path, method) => path === "/api/runners" ? [runner] : path === "/api/runners/runner/credentials" && method === "POST" ? { ...credential, token: "moleapi_runner_secret" } : path === "/api/runners/runner/credentials" ? [credential] : []);
  render(view()); fireEvent.click(screen.getByText("Self-hosted runners"));
  fireEvent.click(await screen.findByRole("button", { name: "Manage credentials" }));
  await screen.findByText("Agent");
  fireEvent.change(screen.getByLabelText("Credential name"), { target: { value: "Agent" } });
  fireEvent.click(screen.getByRole("button", { name: "Create runner credential" }));
  await waitFor(() => expect(mocks.api).toHaveBeenCalledWith("/api/runners/runner/credentials", "POST", { workspace_id: "w", name: "Agent", expires_in_days: 90 }));
  expect(await screen.findByLabelText("New runner credential")).toHaveProperty("value", "moleapi_runner_secret");
  fireEvent.click(screen.getByRole("button", { name: "Close credential manager" }));
  expect(screen.queryByText("moleapi_runner_secret")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Manage credentials" }));
  await screen.findByText("Agent");
  expect(screen.queryByLabelText("New runner credential")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Revoke runner credential" }));
  expect(mocks.api).not.toHaveBeenCalledWith("/api/runners/runner/credentials/cred", "DELETE");
  fireEvent.click(screen.getByRole("button", { name: "Confirm revocation" }));
  await waitFor(() => expect(mocks.api).toHaveBeenCalledWith("/api/runners/runner/credentials/cred", "DELETE"));
});
it("discards late credential issuance after account changes", async () => {
  let resolve!: (value: unknown) => void;
  mocks.api.mockImplementation((path, method) => path === "/api/runners" ? Promise.resolve([runner]) : method === "POST" ? new Promise(done => { resolve = done; }) : Promise.resolve([]));
  const rendered = render(view()); fireEvent.click(screen.getByText("Self-hosted runners"));
  fireEvent.click(await screen.findByRole("button", { name: "Manage credentials" }));
  fireEvent.change(screen.getByLabelText("Credential name"), { target: { value: "Private" } });
  fireEvent.click(screen.getByRole("button", { name: "Create runner credential" }));
  await waitFor(() => expect(mocks.api).toHaveBeenCalledWith("/api/runners/runner/credentials", "POST", expect.anything()));
  mocks.state.accountId = "other"; mocks.credential = "new-session"; rendered.rerender(view());
  await act(async () => resolve({ token: "moleapi_runner_old_secret" }));
  expect(document.body.textContent).not.toContain("moleapi_runner_old_secret");
});
