// @vitest-environment jsdom
import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { api, ApiError } from "../api";
import { errorCopy, liveError, LocalizedError } from "./errors";
import type { ErrorCopy } from "./errors";
import { setLanguage, translateCopy, useLanguage } from "./index";
function Failure({ copy }: { copy: ErrorCopy }) { useLanguage(); return <div role="alert">{translateCopy(copy)}</div>; }
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });
it("retains app-owned interpolation metadata for inline errors and visible toast content", async () => {
  const value = "我的服务 <原文>";
  const failure = new LocalizedError("删除「{{value0}}」，保存后生效。", { value0: value });
  const copy = errorCopy(failure);
  render(<><Failure copy={copy} /><div role="status">{liveError(failure)}</div></>);
  expect(screen.getByRole("alert").textContent).toBe("删除「我的服务 <原文>」，保存后生效。");
  await act(async () => { await setLanguage("en"); });
  expect(screen.getByRole("alert").textContent).toBe("Delete “我的服务 <原文>”. Takes effect when saved.");
  expect(screen.getByRole("status").textContent).toBe(screen.getByRole("alert").textContent);
  expect(failure.message).toBe(screen.getByRole("alert").textContent);
});
it("does not reinterpret a foreign error or plain text matching an app-owned key", async () => {
  const raw = "请先解决工作区保存冲突";
  const error = new Error(raw);
  await setLanguage("en");
  expect(errorCopy(error)).toBe(raw);
  expect(errorCopy(raw)).toBe(raw);
  expect(liveError(error)).toBe(raw);
});
it.each([false, true])("preserves local HTTP fallback provenance through a caught error (malformed JSON: %s)", async malformed => {
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ status: 503, json: malformed ? async () => { throw new SyntaxError("invalid"); } : async () => ({}) }));
  const failure = await api("/api/example").catch(error => error);
  expect(failure).toBeInstanceOf(ApiError);
  expect((failure as ApiError).status).toBe(503);
  render(<Failure copy={errorCopy(failure)} />);
  expect(screen.getByRole("alert").textContent).toBe(malformed ? "服务返回了无法解析的数据 (503)" : "请求失败 (503)");
  await act(async () => { await setLanguage("en"); });
  expect(screen.getByRole("alert").textContent).toBe(malformed ? "The server returned unreadable data (503)" : "Request failed (503)");
});
it("keeps an actual server HTTP error literal across locale changes", async () => {
  const raw = "请求失败 (503)";
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ status: 503, json: async () => ({ error: raw }) }));
  const failure = await api("/api/example").catch(error => error);
  expect(failure).toBeInstanceOf(ApiError);
  render(<Failure copy={errorCopy(failure)} />);
  await act(async () => { await setLanguage("en"); });
  expect(screen.getByRole("alert").textContent).toBe(raw);
});
