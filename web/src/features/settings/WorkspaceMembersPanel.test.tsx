// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Theme } from "@radix-ui/themes";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { api, token } from "../../shared/api";
import { setLanguage } from "../../shared/i18n";
import WorkspaceMembersPanel from "./WorkspaceMembersPanel";

vi.mock("../../shared/api", () => ({ api: vi.fn(), token: vi.fn() }));
vi.mock("../../shared/ui", () => ({ Choice: ({ label, value, options, onChange }: { label: string; value: string; options: { value: string; label: string }[]; onChange: (value: string) => void }) => <label>{label}<select value={value} onChange={(event) => onChange(event.target.value)}>{options.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</select></label> }));

let client: QueryClient;
function view(accountId = "owner", ownerId?: string) { return <QueryClientProvider client={client}><Theme><WorkspaceMembersPanel accountId={accountId} workspaceId="w" ownerId={ownerId} /></Theme></QueryClientProvider>; }
beforeEach(async () => {
  await setLanguage("zh-CN");
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  vi.mocked(api).mockReset();
  vi.mocked(token).mockReturnValue("session-owner");
  vi.mocked(api).mockImplementation((path, method) => {
    if (path === "/api/auth/me") return Promise.resolve({ id: "owner-id", username: "owner" });
    if (path.endsWith("/members") && (method ?? "GET") === "GET") return Promise.resolve([
      { member_id: "owner-id", username: "owner", role: "owner", revision: 0 },
      { member_id: "member-id", username: "memberone", role: "editor", revision: 2 },
    ]);
    if (path.endsWith("/invitations") && (method ?? "GET") === "GET") return Promise.resolve([]);
    if (path.endsWith("/invitations") && method === "POST") return Promise.resolve({ id: "invite-one", token: "wi_one-time-secret" });
    return Promise.resolve({});
  });
  Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: vi.fn().mockResolvedValue(undefined) } });
});
afterEach(() => { cleanup(); client.clear(); });

it("shows an invitation only once and never stores its token in query cache", async () => {
  const rendered = render(view());
  await screen.findByText("memberone");
  fireEvent.click(screen.getByRole("button", { name: "创建邀请链接" }));
  const invitation = await screen.findByLabelText("新邀请链接") as HTMLTextAreaElement;
  expect(invitation.value).toContain("workspace_invite=wi_one-time-secret");
  expect(JSON.stringify(client.getQueryCache().getAll().map((query) => query.state.data))).not.toContain("wi_one-time-secret");
  fireEvent.click(screen.getByRole("button", { name: "复制邀请链接" }));
  await waitFor(() => expect(navigator.clipboard.writeText).toHaveBeenCalledWith(invitation.value));
  vi.mocked(token).mockReturnValue("session-replaced");
  rendered.rerender(view());
  await waitFor(() => expect(screen.queryByLabelText("新邀请链接")).toBeNull());
});

it("updates a member with revision checking and confirms removal", async () => {
  render(view());
  await screen.findByText("memberone");
  fireEvent.change(screen.getByLabelText("memberone 成员角色"), { target: { value: "viewer" } });
  fireEvent.click(screen.getByRole("button", { name: "更新角色" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith("/api/owners/owner-id/workspaces/w/members/member-id", "PATCH", { role: "viewer", expected_revision: 2 }));
  fireEvent.click(screen.getByRole("button", { name: "移除成员" }));
  expect(api).not.toHaveBeenCalledWith("/api/owners/owner-id/workspaces/w/members/member-id", "DELETE");
  await act(async () => fireEvent.click(screen.getByRole("button", { name: "确认" })));
  await waitFor(() => expect(api).toHaveBeenCalledWith("/api/owners/owner-id/workspaces/w/members/member-id", "DELETE"));
});

it("uses the canonical owner path for an admin member", async () => {
  render(view("admin-account", "owner-id"));
  await screen.findByText("memberone");
  fireEvent.click(screen.getByRole("button", { name: "创建邀请链接" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith("/api/owners/owner-id/workspaces/w/invitations", "POST", { role: "editor", expires_in_days: 7 }));
  expect(api).not.toHaveBeenCalledWith("/api/auth/me");
});
