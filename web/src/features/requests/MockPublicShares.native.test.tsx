// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { Theme } from "@radix-ui/themes";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { setLanguage } from "../../shared/i18n";
import MockPublicShares from "./MockPublicShares";

const api = vi.hoisted(() => vi.fn());
vi.mock("../../shared/api", () => ({ api, native: true, token: () => null }));
beforeEach(() => { api.mockReset(); vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} }); });
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });

it("requires an explicitly started native listener and uses its actual origin for one-time URLs", async () => {
  await setLanguage("en");
  let active = false;
  const status = () => ({ active, origin: active ? "http://127.0.0.1:18974" : null, bind: active ? "127.0.0.1:18974" : null });
  api.mockImplementation(async (path: string, method = "GET") => {
    if (path === "/api/mock-public/listener/start") { active = true; return status(); }
    if (path === "/api/mock-public/listener/stop") { active = false; return status(); }
    if (path === "/api/mock-public/listener") return status();
    if (method === "GET") return [];
    return { id: "share", request_id: "r", expires_at: 2_000_000_000, allow_dynamic: false, stale: false, token: "msp_native" };
  });
  render(<Theme><MockPublicShares workspaceId="w" requestId="r" examples={[{ id: "e", name: "Example", dynamic: false }]} routePath="/items" hasSavedQuery={false} dirty={false} /></Theme>);
  expect(await screen.findByText("The local listener is stopped")).toBeTruthy();
  expect(screen.getByRole("button", { name: "Create public URL" })).toHaveProperty("disabled", true);
  fireEvent.click(screen.getByRole("button", { name: "Start listener" }));
  expect(await screen.findByText("Listening at http://127.0.0.1:18974")).toBeTruthy();
  expect(api).toHaveBeenCalledWith("/api/mock-public/listener/start", "POST", { host: "127.0.0.1", port: 0, allow_lan: false });
  expect(screen.getByRole("button", { name: "Create public URL" })).toHaveProperty("disabled", false);
  fireEvent.click(screen.getByRole("button", { name: "Create public URL" }));
  expect((await screen.findAllByText(/127\.0\.0\.1:18974\/api\/public\/mock/)).length).toBeGreaterThan(0);
  fireEvent.click(screen.getByRole("button", { name: "Stop listener" }));
  expect(await screen.findByText("The local listener is stopped")).toBeTruthy();
  expect(screen.queryByText(/msp_native/)).toBeNull();
  expect(screen.getByRole("button", { name: "Create public URL" })).toHaveProperty("disabled", true);
});
