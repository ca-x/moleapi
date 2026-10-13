// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { Theme } from "@radix-ui/themes";
import { afterEach, expect, it, vi } from "vitest";
import { setLanguage } from "../../shared/i18n";
import SettingsPanel from "./SettingsPanel";

const mocks = vi.hoisted(() => ({ api: vi.fn(), connected: false }));
vi.mock("../../shared/api", () => ({ api: mocks.api, native: true, token: () => null }));
vi.mock("../workbench/context", () => ({ useWorkbench: () => ({ accountId: "local", draft: null, sync: { data: { connected: mocks.connected } }, disconnectServer: vi.fn(), workspaces: {}, dark: false, logout: vi.fn() }) }));
vi.mock("../../shared/i18n/LanguageSelector", () => ({ default: () => null }));
afterEach(() => { cleanup(); mocks.api.mockReset(); mocks.connected = false; });

it("does not mount the hosted runner gateway when native sync is disconnected", async () => {
  await setLanguage("en");
  render(<Theme><SettingsPanel /></Theme>);
  expect(screen.queryByText("Hosted runner management")).toBeNull();
  expect(mocks.api).not.toHaveBeenCalled();
});
