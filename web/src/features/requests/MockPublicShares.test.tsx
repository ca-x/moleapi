// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { Theme } from "@radix-ui/themes";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { setLanguage } from "../../shared/i18n";
import MockPublicShares from "./MockPublicShares";

const api = vi.hoisted(() => vi.fn());
vi.mock("../../shared/api", () => ({ api, native: false, token: () => "owner-session" }));
beforeEach(() => api.mockReset());
afterEach(cleanup);

it("shows public URLs once, then rotates and revokes the saved share", async () => {
  await setLanguage("en");
  let rows: { id: string; request_id: string; expires_at: number; allow_dynamic: boolean; stale: boolean }[] = [];
  api.mockImplementation(async (_path: string, method = "GET") => {
    if (method === "GET") return rows;
    if (method === "POST" && _path.endsWith("/rotate")) return { ...rows[0], token: "msp_rotated" };
    if (method === "POST") {
      rows = [{ id: "share-1", request_id: "r", expires_at: 2_000_000_000, allow_dynamic: false, stale: false }];
      return { ...rows[0], token: "msp_first" };
    }
    rows = [];
    return { revoked: true };
  });
  const props = { workspaceId: "w", requestId: "r", examples: [{ id: "e", name: "Example", dynamic: false }], routePath: "/items", hasSavedQuery: false, dirty: false };
  const page = render(<Theme><MockPublicShares {...props} /></Theme>);
  expect(await screen.findByText("No public URLs for this request")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Create public URL" }));
  expect(await screen.findByText(/Save these URLs now/)).toBeTruthy();
  expect(screen.getAllByText(/msp_first/).length).toBeGreaterThan(0);
  expect(api).toHaveBeenCalledWith("/api/workspaces/w/mock-shares", "POST", { request_id: "r", expires_in_days: 7, allow_dynamic: false });
  fireEvent.click(await screen.findByRole("button", { name: "Rotate URL" }));
  fireEvent.click(screen.getByRole("button", { name: "Confirm rotation and disable old URL" }));
  expect((await screen.findAllByText(/msp_rotated/)).length).toBeGreaterThan(0);
  expect(screen.queryByText(/msp_first/)).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Close one-time URLs" }));
  expect(screen.queryByText(/msp_rotated/)).toBeNull();
  page.unmount();
  render(<Theme><MockPublicShares {...props} /></Theme>);
  expect(await screen.findByRole("button", { name: "Rotate URL" })).toBeTruthy();
  expect(screen.queryByText(/msp_rotated/)).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Revoke URL" }));
  fireEvent.click(screen.getByRole("button", { name: "Confirm revoke public URL" }));
  await waitFor(() => expect(screen.getByText("No public URLs for this request")).toBeTruthy());
});

it("requires a saved draft and valid expiry while explaining dynamic access in Chinese", async () => {
  await setLanguage("zh-CN");
  api.mockResolvedValue([]);
  render(<Theme><MockPublicShares workspaceId="w" requestId="r" examples={[{ id: "e", name: "示例", dynamic: false }]} routePath="/items" hasSavedQuery={false} dirty /></Theme>);
  expect(screen.getByText("先保存工作区，再创建公开地址。已有地址仍可撤销。")).toBeTruthy();
  expect(screen.getByRole("button", { name: "创建公开地址" })).toHaveProperty("disabled", true);
  fireEvent.change(screen.getByRole("spinbutton", { name: "有效期（天）" }), { target: { value: "91" } });
  expect(screen.getByRole("alert").textContent).toContain("1–90");
  fireEvent.click(screen.getByRole("checkbox", { name: "允许公开地址运行 Mock 脚本" }));
  expect(screen.getByText("Mock 脚本可能读取工作区数据并将其返回给公开调用者。")).toBeTruthy();
  expect(api).not.toHaveBeenCalledWith(expect.anything(), "POST", expect.anything());
});

it("does not offer a ready-to-copy routed URL when saved query conditions are required", async () => {
  await setLanguage("en");
  api.mockImplementation(async (_path: string, method = "GET") => method === "GET" ? [] : {
    id: "share-1", request_id: "r", expires_at: 2_000_000_000,
    allow_dynamic: false, stale: false, token: "msp_first",
  });
  render(<Theme><MockPublicShares workspaceId="w" requestId="r" examples={[{ id: "e", name: "Example", dynamic: false }, { id: "script", name: "Script", dynamic: true }]} routePath="/items" hasSavedQuery dirty={false} /></Theme>);
  await screen.findByText("No public URLs for this request");
  fireEvent.click(screen.getByRole("button", { name: "Create public URL" }));
  expect(await screen.findByText(/This request needs query parameters/)).toBeTruthy();
  expect(screen.getAllByText(/msp_first/)).toHaveLength(1);
  expect(screen.queryByText("Public matching URL")).toBeNull();
});

it("does not create a dead static share for a request with only dynamic examples", async () => {
  await setLanguage("en");
  api.mockResolvedValue([]);
  render(<Theme><MockPublicShares workspaceId="w" requestId="r" examples={[{ id: "script", name: "Script", dynamic: true }]} routePath="/items" hasSavedQuery={false} dirty={false} /></Theme>);
  expect(await screen.findByText(/only dynamic examples/)).toBeTruthy();
  expect(screen.getByRole("button", { name: "Create public URL" })).toHaveProperty("disabled", true);
  fireEvent.click(screen.getByRole("checkbox", { name: "Allow Mock scripts on this public URL" }));
  expect(screen.getByRole("button", { name: "Create public URL" })).toHaveProperty("disabled", false);
});
