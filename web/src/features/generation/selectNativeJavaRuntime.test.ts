import {expect, it, vi} from "vitest";
import {selectNativeJavaRuntime} from "./selectNativeJavaRuntime";
const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({invoke}));
it("uses the native dialog command without accepting an executable path", async () => {
  invoke.mockResolvedValueOnce(null).mockResolvedValueOnce(21);
  expect(await selectNativeJavaRuntime()).toBeNull();
  expect(await selectNativeJavaRuntime()).toBe(21);
  expect(invoke.mock.calls).toEqual([["select_java_runtime"], ["select_java_runtime"]]);
});
