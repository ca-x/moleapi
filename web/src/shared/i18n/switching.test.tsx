// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { Theme } from "@radix-ui/themes";
import { afterEach, expect, it, vi } from "vitest";
import AuthScreen from "../../features/auth/AuthScreen";
import RequestEditor from "../../features/requests/RequestEditor";
import NavigationRail from "../../features/workbench/NavigationRail";
import GuardDialog from "../../features/workspaces/GuardDialog";
import { WorkbenchContext } from "../../features/workbench/context";
import type { useWorkbenchController } from "../../features/workbench/useWorkbenchController";
import { initialData, newRequest } from "../model";
import { api } from "../api";
import { liveTranslation, message, setLanguage } from "./index";
import LanguageSelector from "./LanguageSelector";
vi.mock("../api", () => ({ api: vi.fn(), native: false }));
// Keep payload editors visible in jsdom without relying on browser layout APIs.
vi.mock("../ui", async (original) => ({
  ...(await original<typeof import("../ui")>()),
  Editor: ({ value, label, readOnly, onChange }: { value: string; label: string; readOnly?: boolean; onChange?: (value: string) => void }) =>
    <textarea aria-label={label} value={value} readOnly={readOnly} onChange={event => onChange?.(event.target.value)} />,
}));
vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} });
Element.prototype.scrollIntoView = vi.fn();
afterEach(() => { cleanup(); vi.clearAllMocks(); });
const theme = ({ children }: { children: React.ReactNode }) => <Theme>{children}</Theme>;
function context(state: unknown, children: React.ReactNode) {
  return <Theme><WorkbenchContext.Provider value={state as ReturnType<typeof useWorkbenchController>}>{children}</WorkbenchContext.Provider></Theme>;
}
it("switches the actual authentication selector while preserving credentials, focus, and auth flow", async () => {
  vi.mocked(api).mockResolvedValue({ token: "session-token", username: "我的账户" });
  const login = vi.fn();
  render(<AuthScreen status={{ mode: "server", setup_required: false, registration_enabled: false }} onLogin={login} />, { wrapper: theme });
  const username = screen.getByLabelText("用户名") as HTMLInputElement;
  const password = screen.getByLabelText("密码") as HTMLInputElement;
  fireEvent.change(username, { target: { value: "我的账户" } });
  fireEvent.change(password, { target: { value: "我的私密密码" } });
  // Open Radix’s real selector rather than replacing its behavior in the test.
  fireEvent.click(screen.getByRole("combobox", { name: "界面语言" }));
  await act(async () => { fireEvent.click(screen.getByRole("option", { name: "English" })); });
  expect(screen.getByRole("heading", { name: "Sign in to your workbench" })).toBeTruthy();
  expect(screen.getByLabelText("Username")).toBe(username);
  expect(username.value).toBe("我的账户");
  expect(password.value).toBe("我的私密密码");
  await waitFor(() => expect(screen.getByRole("combobox", { name: "Interface language" })).toBe(document.activeElement));
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Sign in" })); });
  expect(api).toHaveBeenCalledWith("/api/auth/login", "POST", { username: "我的账户", password: "我的私密密码", setup_token: "" });
  expect(login).toHaveBeenCalledTimes(1);
});
it("keeps an exact server error in both languages", async () => {
  vi.mocked(api).mockRejectedValue(new Error("服务端原始错误: credentials rejected"));
  render(<AuthScreen status={{ mode: "server", setup_required: false, registration_enabled: false }} onLogin={vi.fn()} />, { wrapper: theme });
  fireEvent.change(screen.getByLabelText("用户名"), { target: { value: "valid-account" } });
  fireEvent.change(screen.getByLabelText("密码"), { target: { value: "valid-password" } });
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "登录" })); });
  expect(screen.getByRole("alert").textContent).toBe("服务端原始错误: credentials rejected");
  await act(async () => { await setLanguage("en"); });
  expect(screen.getByRole("alert").textContent).toBe("服务端原始错误: credentials rejected");
});
it("switches the complete request toolbar and tabs without changing user-owned fields or payloads", async () => {
  const request = { ...newRequest("我的接口", "https://example.com/中文?q=原文"), description: "我的接口说明", body_kind: "json" as const, body: '{"文字":"不翻译"}', scripts: { pre_request: "// 我的脚本", post_response: "// 响应脚本" } };
  const before = JSON.stringify(request), update = vi.fn();
  render(<RequestEditor request={request} update={update} send={vi.fn()} save={vi.fn()} dirty saving={false} busy={false} sending={false} dark={false} response={null} error="" />, { wrapper: theme });
  const name = screen.getByLabelText("请求名称") as HTMLInputElement;
  name.focus();
  await act(async () => { await setLanguage("en"); });
  expect(screen.getByRole("tab", { name: "Body" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Generate code" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Send" })).toBeTruthy();
  expect(screen.getByLabelText("Request name")).toBe(name);
  expect(document.activeElement).toBe(name);
  expect(name.value).toBe("我的接口");
  expect((screen.getByLabelText("Request URL") as HTMLInputElement).value).toBe(request.url);
  expect(JSON.stringify(request)).toBe(before);
  expect(update).not.toHaveBeenCalled();
  fireEvent.mouseDown(screen.getByRole("tab", { name: "Docs" }), { button: 0 });
  expect((screen.getByLabelText("API documentation") as HTMLTextAreaElement).value).toBe(request.description);
  await act(async () => { await setLanguage("zh-CN"); });
  expect((screen.getByLabelText("接口说明") as HTMLTextAreaElement).value).toBe(request.description);
  expect(JSON.stringify(request)).toBe(before);
});
it("updates navigation and a pending interpolated guard without changing stored sample names", async () => {
  const data = initialData(), before = JSON.stringify(data);
  const state = { draft: { id: "w", name: "我的工作区", data }, view: "requests", setView: vi.fn(), setHistoryResponse: vi.fn(), openModal: vi.fn(), guard: { title: message("删除环境"), description: message("删除「{{value0}}」，保存后生效。", { value0: "我的环境" }), action: vi.fn() }, saving: false, setGuard: vi.fn(), confirmGuard: vi.fn() };
  render(context(state, <NavigationRail />));
  await act(async () => { await setLanguage("en"); });
  expect(screen.getByRole("complementary", { name: "Module navigation" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Environments" })).toBeTruthy();
  await act(async () => { await setLanguage("zh-CN"); });
  render(context(state, <GuardDialog />));
  await act(async () => { await setLanguage("en"); });
  expect(screen.getByRole("alertdialog", { name: "Delete environment" }).textContent).toContain("Delete “我的环境”. Takes effect when saved.");
  expect(JSON.stringify(data)).toBe(before);
  expect(initialData().collections[0].name).toBe("Quick start");
});
it("updates existing toast content and retains interpolated user text exactly", async () => {
  render(liveTranslation("删除「{{value0}}」，保存后生效。", { value0: "我的环境" }));
  expect(screen.getByText("删除「我的环境」，保存后生效。")).toBeTruthy();
  await act(async () => { await setLanguage("en"); });
  expect(screen.getByText("Delete “我的环境”. Takes effect when saved.")).toBeTruthy();
});
it("switches the selector even when browser storage is denied", async () => {
  const store = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw new DOMException("denied"); });
  render(<LanguageSelector />, { wrapper: theme });
  await act(async () => { await setLanguage("en"); });
  expect(screen.getByRole("combobox", { name: "Interface language" }).textContent).toContain("English");
  store.mockRestore();
});
