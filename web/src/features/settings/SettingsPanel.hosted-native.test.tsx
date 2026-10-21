// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Theme } from "@radix-ui/themes";
import { afterEach, expect, it, vi } from "vitest";
import { setLanguage } from "../../shared/i18n";
import SettingsPanel from "./SettingsPanel";

const mocks = vi.hoisted(() => ({ api: vi.fn(), connected: false, draft: null as null | {name:string;data:{script_mode?:"v1"|"v2"}}, updateData: vi.fn() }));
vi.mock("../../shared/api", () => ({ api: mocks.api, native: true, token: () => null }));
vi.mock("../workbench/context", () => ({ useWorkbench: () => ({ accountId: "local", draft: mocks.draft, updateData: mocks.updateData, sync: { data: { connected: mocks.connected } }, disconnectServer: vi.fn(), workspaces: {}, dark: false, logout: vi.fn() }) }));
vi.mock("../../shared/i18n/LanguageSelector", () => ({ default: () => null }));
vi.mock("../ci/CiPanel", () => ({ default: () => null }));
vi.mock("../authentication/RequestAuthEditor", () => ({ default: () => null }));
afterEach(() => { cleanup(); mocks.api.mockReset(); mocks.updateData.mockReset(); mocks.connected = false; mocks.draft = null; });

it("does not mount the hosted runner gateway when native sync is disconnected", async () => {
  await setLanguage("en");
  render(<QueryClientProvider client={new QueryClient()}><Theme><SettingsPanel /></Theme></QueryClientProvider>);
  expect(screen.queryByText("Hosted runner management")).toBeNull();
  expect(mocks.api).not.toHaveBeenCalled();
});

it("keeps old workspaces in v1 and saves an explicit v2 choice", async () => {
  await setLanguage("en");
  vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} });
  Object.defineProperty(HTMLElement.prototype, "scrollIntoView", { configurable: true, value: vi.fn() });
  mocks.draft = { name: "Legacy", data: {} };
  mocks.updateData.mockImplementation((update: (data: {script_mode?:"v1"|"v2"}) => {script_mode?:"v1"|"v2"}) => {
    mocks.draft!.data = update(mocks.draft!.data);
  });
  const page = render(<QueryClientProvider client={new QueryClient()}><Theme><SettingsPanel /></Theme></QueryClientProvider>);
  const selector = screen.getByRole("combobox", { name: "Script compatibility mode" });
  expect(selector.textContent).toContain("v1");
  fireEvent.click(selector);
  fireEvent.click(screen.getByRole("option", { name: "v2 (async scripts)" }));
  expect(mocks.draft.data.script_mode).toBe("v2");
  page.rerender(<QueryClientProvider client={new QueryClient()}><Theme><SettingsPanel /></Theme></QueryClientProvider>);
  expect(screen.getByRole("combobox", { name: "Script compatibility mode" }).textContent).toContain("v2");
});
