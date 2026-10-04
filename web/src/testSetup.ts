import { beforeEach, vi } from "vitest";
import { i18n } from "./shared/i18n";
if (typeof window !== "undefined") {
  // Node 26 exposes an undefined Web Storage global that Vitest 3 copies over
  // jsdom’s getter. Use genuine jsdom storage so tests still exercise its API.
  const { JSDOM } = await import("jsdom");
  const storageWindow = new JSDOM("", { url: window.location.href }).window;
  vi.stubGlobal("localStorage", storageWindow.localStorage);
  vi.stubGlobal("sessionStorage", storageWindow.sessionStorage);
  vi.stubGlobal("Storage", storageWindow.Storage);
}
// Existing behavior tests assert the original Chinese copy explicitly. Every test
// starts in that locale; localization tests switch to English themselves.
beforeEach(async () => { await i18n.changeLanguage("zh-CN"); });
