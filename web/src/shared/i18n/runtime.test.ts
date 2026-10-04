// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { i18n, LANGUAGE_STORAGE_KEY, resolveLanguage, setLanguage, t } from "./index";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
afterEach(() => { delete (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__; vi.restoreAllMocks(); localStorage.clear(); });
describe("language preference", () => {
  it("uses supported saved choices before browser detection", () => {
    expect(resolveLanguage("en", "zh-CN")).toBe("en");
    expect(resolveLanguage("zh-CN", "en-US")).toBe("zh-CN");
    expect(resolveLanguage(null, "zh-TW")).toBe("zh-CN");
    expect(resolveLanguage("fr", "en-US")).toBe("en");
    expect(resolveLanguage(null, "fr-FR")).toBe("en");
  });
  it("persists a choice and changes document metadata immediately", async () => {
    await setLanguage("en");
    expect(localStorage.getItem(LANGUAGE_STORAGE_KEY)).toBe("en");
    expect(document.documentElement.lang).toBe("en");
    expect(document.title).toBe("MoleAPI · API Workbench");
    await setLanguage("zh-CN");
    expect(document.documentElement.lang).toBe("zh-CN");
    expect(document.title).toBe("MoleAPI · API 工作台");
  });
  it("keeps switching in memory when storage writes are denied", async () => {
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw new DOMException("denied"); });
    await expect(setLanguage("en")).resolves.toBeUndefined();
    expect(i18n.language).toBe("en");
    expect(t("保存工作区")).toBe("Save workspace");
  });
  it("initializes without storage access and honors a saved supported preference on reload", async () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => { throw new DOMException("denied"); });
    vi.spyOn(navigator, "language", "get").mockReturnValue("zh-CN");
    vi.resetModules();
    const denied = await import("./index");
    expect(denied.i18n.language).toBe("zh-CN");
    vi.restoreAllMocks();
    localStorage.setItem(LANGUAGE_STORAGE_KEY, "en");
    vi.spyOn(navigator, "language", "get").mockReturnValue("zh-CN");
    vi.resetModules();
    const restored = await import("./index");
    expect(restored.i18n.language).toBe("en");
  });
});
it("uses interpolation without treating punctuation or namespaces as key separators", async () => {
  await setLanguage("en");
  expect(t("GraphQL 服务返回非 JSON 内容（HTTP {{value0}}）", { value0: 503 })).toBe("GraphQL server returned non-JSON content (HTTP 503)");
  expect(t("删除「{{value0}}」，保存后生效。", { value0: "我的接口 <原文>" })).toBe("Delete “我的接口 <原文>”. Takes effect when saved.");
  expect(t("{{count}} 个请求", { count: 1 })).toBe("1 request");
  expect(t("{{count}} 个请求", { count: 2 })).toBe("2 requests");
  expect(t("可使用 {{api_token}} 引用环境变量。")).toBe("Use {{api_token}} to reference an environment variable.");
});

it("sends the trusted preference to native IPC without blocking switches on bridge errors", async () => {
  const { invoke } = await import("@tauri-apps/api/core");
  (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {};
  await setLanguage("en");
  expect(invoke).toHaveBeenCalledWith("set_language", { language: "en" });
  vi.mocked(invoke).mockRejectedValueOnce(new Error("No tray"));
  await expect(setLanguage("zh-CN")).resolves.toBeUndefined();
  expect(i18n.language).toBe("zh-CN");
});
