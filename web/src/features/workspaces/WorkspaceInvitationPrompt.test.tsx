// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Theme } from "@radix-ui/themes";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { api, token } from "../../shared/api";
import { setLanguage } from "../../shared/i18n";
import WorkspaceInvitationPrompt from "./WorkspaceInvitationPrompt";

vi.mock("../../shared/api", () => ({ api: vi.fn(), token: vi.fn(), native: false }));
let client: QueryClient;
beforeEach(async () => {
  await setLanguage("zh-CN");
  window.history.replaceState(null, "", "/#workspace_invite=wi_one-time-secret");
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  vi.mocked(token).mockReturnValue("session-member");
  vi.mocked(api).mockReset();
  vi.mocked(api).mockResolvedValue({ owner_id: "owner", workspace_id: "w", role: "viewer" });
});
afterEach(() => { cleanup(); client.clear(); window.history.replaceState(null, "", "/"); });

it("accepts the one-time invitation for the current account and clears the URL token", async () => {
  render(<QueryClientProvider client={client}><Theme><WorkspaceInvitationPrompt accountId="member" /></Theme></QueryClientProvider>);
  fireEvent.click(screen.getByRole("button", { name: "接受邀请" }));
  await waitFor(() => expect(api).toHaveBeenCalledWith("/api/workspace-invitations/accept", "POST", { token: "wi_one-time-secret" }));
  await waitFor(() => expect(window.location.hash).toBe(""));
  expect(screen.queryByText("加入共享工作区")).toBeNull();
});

it("does not finish an invitation under a replaced session", async () => {
  let finish!: (value: unknown) => void;
  vi.mocked(api).mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
  const rendered = render(<QueryClientProvider client={client}><Theme><WorkspaceInvitationPrompt accountId="member" /></Theme></QueryClientProvider>);
  fireEvent.click(screen.getByRole("button", { name: "接受邀请" }));
  await waitFor(() => expect(api).toHaveBeenCalled());
  vi.mocked(token).mockReturnValue("another-session");
  rendered.rerender(<QueryClientProvider client={client}><Theme><WorkspaceInvitationPrompt accountId="member" /></Theme></QueryClientProvider>);
  await act(async () => finish({ owner_id: "owner", workspace_id: "w" }));
  expect(window.location.hash).toContain("workspace_invite=");
});

it("opens an invitation received while the workbench is already mounted", async () => {
  window.history.replaceState(null, "", "/");
  render(<QueryClientProvider client={client}><Theme><WorkspaceInvitationPrompt accountId="member" /></Theme></QueryClientProvider>);
  expect(screen.queryByRole("button", { name: "接受邀请" })).toBeNull();
  window.history.replaceState(null, "", "/#workspace_invite=wi_later");
  fireEvent(window, new Event("hashchange"));
  expect(await screen.findByRole("button", { name: "接受邀请" })).toBeTruthy();
});
