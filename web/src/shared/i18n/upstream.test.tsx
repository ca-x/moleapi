// @vitest-environment jsdom
import { act, cleanup, render, screen } from "@testing-library/react";
import { ExplorerSection } from "@graphiql/plugin-doc-explorer";
import { afterEach, expect, it } from "vitest";
import { setLanguage } from "./index";
import monaco from "./monaco-zh-CN.json";
afterEach(cleanup);
it("localizes GraphiQL documentation headings while retaining stable icon keys", async () => {
  render(<ExplorerSection title="Fields"><span>我的字段说明</span></ExplorerSection>);
  expect(screen.getByText("字段")).toBeTruthy();
  expect(screen.getByText("我的字段说明")).toBeTruthy();
  await act(async () => { await setLanguage("en"); });
  expect(screen.getByText("Fields")).toBeTruthy();
  expect(screen.getByText("我的字段说明")).toBeTruthy();
});
it("embeds official Monaco NLS labels, positional templates, and the missing inline-edit label", () => {
  expect(Object.keys(monaco).length).toBeGreaterThan(1700);
  expect(monaco["Type to narrow down results."]).toBe("在此输入可缩小结果范围。");
  expect(monaco["Fold Level {0}"]).toBe("折叠级别 {0}");
  expect(monaco["Trigger Inline Edit"]).toBe("触发行内编辑");
});
it("keeps Monaco command labels and positional values correct through repeat switches", async () => {
  const { localize, localize2, localizeFixedLabel, onLanguageChange } = await import("monaco-editor/esm/vs/nls.js");
  const descriptor = localize2("find", "Find");
  expect(descriptor.value).toBe("查找");
  expect(descriptor.original).toBe("Find");
  expect(localize("find", "Find")).toBe("查找");
  expect(localizeFixedLabel("Fold Level 3")).toBe("折叠级别 3");
  expect(localizeFixedLabel("Close (Escape)")).toBe("关闭 (Escape)");
  let updates = 0;
  const listener = onLanguageChange(() => { updates++; });
  await setLanguage("en");
  expect(descriptor.value).toBe("Find");
  expect(localizeFixedLabel("关闭 (Escape)")).toBe("Close (Escape)");
  expect(localizeFixedLabel("折叠级别 3")).toBe("Fold Level 3");
  expect(localizeFixedLabel("Copy Query")).toBe("Copy Query");
  await setLanguage("zh-CN");
  expect(localizeFixedLabel("Copy Query")).toBe("复制查询");
  expect(updates).toBe(3);
  listener.dispose();
  await setLanguage("en");
  expect(updates).toBe(3);
});
