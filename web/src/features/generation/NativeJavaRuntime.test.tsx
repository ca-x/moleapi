// @vitest-environment jsdom
import {act, cleanup, fireEvent, render, screen, waitFor} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach, beforeEach, expect, it, vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import NativeJavaRuntime from "./NativeJavaRuntime";
const mocks = vi.hoisted(() => ({select: vi.fn(), api: vi.fn()}));
vi.mock("./selectNativeJavaRuntime", () => ({selectNativeJavaRuntime: mocks.select}));
vi.mock("../../shared/api", () => ({api: mocks.api}));
beforeEach(async () => { await setLanguage("en"); mocks.select.mockReset(); mocks.api.mockReset(); });
afterEach(cleanup);

it("refreshes catalog after native selection and prevents duplicate actions", async () => {
  let finish!: (value: number) => void;
  mocks.select.mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  const catalog = {java_available: true, targets: []};
  mocks.api.mockResolvedValue(catalog);
  const updated = vi.fn();
  render(<Theme><NativeJavaRuntime available={false} disabled={false} onConfigured={updated}/></Theme>);
  const button = screen.getByRole("button", {name: "Select Java…"});
  fireEvent.click(button);
  fireEvent.click(button);
  expect(mocks.select).toHaveBeenCalledTimes(1);
  expect(button.hasAttribute("disabled")).toBe(true);
  expect(screen.getByRole("status").textContent).toContain("Selecting and checking Java");
  await act(async () => finish(21));
  await waitFor(() => expect(updated).toHaveBeenCalledWith(catalog));
  expect(mocks.api).toHaveBeenCalledWith("/api/generation/projects/catalog");
});

it("keeps cancellation quiet and shows a local error without changing readiness", async () => {
  mocks.select.mockResolvedValueOnce(null).mockRejectedValueOnce(new Error("private native path"));
  const updated = vi.fn();
  render(<Theme><NativeJavaRuntime available disabled={false} onConfigured={updated}/></Theme>);
  fireEvent.click(screen.getByRole("button", {name: "Change Java…"}));
  await waitFor(() => expect(screen.getByRole("button", {name: "Change Java…"}).hasAttribute("disabled")).toBe(false));
  expect(screen.queryByRole("alert")).toBeNull();
  expect(mocks.api).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", {name: "Change Java…"}));
  expect(await screen.findByRole("alert")).toHaveProperty("textContent", expect.stringContaining("Could not configure Java"));
  expect(screen.queryByText("private native path")).toBeNull();
  expect(updated).not.toHaveBeenCalled();
});

it("discards a selection reply after its source boundary unmounts", async () => {
  let finish!: (value: number) => void;
  mocks.select.mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  const updated = vi.fn();
  const view = render(<Theme><NativeJavaRuntime available={false} disabled={false} onConfigured={updated}/></Theme>);
  fireEvent.click(screen.getByRole("button", {name: "Select Java…"}));
  view.unmount();
  await act(async () => finish(21));
  expect(mocks.api).not.toHaveBeenCalled();
  expect(updated).not.toHaveBeenCalled();
});

it("discards a catalog reply after close and translates the current native state", async () => {
  mocks.select.mockResolvedValue(21);
  let finish!: (value: unknown) => void;
  mocks.api.mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  const updated = vi.fn();
  const view = render(<Theme><NativeJavaRuntime available={false} disabled={false} onConfigured={updated}/></Theme>);
  fireEvent.click(screen.getByRole("button", {name: "Select Java…"}));
  await waitFor(() => expect(finish).toBeTypeOf("function"));
  view.unmount();
  await act(async () => finish({java_available: true}));
  expect(updated).not.toHaveBeenCalled();
  await setLanguage("zh-CN");
  render(<Theme><NativeJavaRuntime available disabled onConfigured={updated}/></Theme>);
  expect(screen.getByRole("button", {name: "更换 Java…"}).hasAttribute("disabled")).toBe(true);
  expect(screen.getByRole("status").textContent).toContain("Java 已配置");
});
