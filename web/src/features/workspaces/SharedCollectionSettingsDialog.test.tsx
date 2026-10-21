// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { Theme } from "@radix-ui/themes";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { initialData } from "../../shared/model";
import { setLanguage } from "../../shared/i18n";
import type { Pair, WorkspaceData } from "../../shared/types";
import SharedCollectionSettingsDialog from "./SharedCollectionSettingsDialog";

vi.mock("../../shared/ui", () => ({
  Field: ({ label, children }: { label: string; children: React.ReactNode }) => <label>{label}{children}</label>,
  Choice: ({ label, value, options, onChange }: { label: string; value: string; options: { value: string; label: string }[]; onChange: (value: string) => void }) => <label>{label}<select value={value} onChange={(event) => onChange(event.target.value)}>{options.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</select></label>,
  Editor: ({ label, value, onChange }: { label: string; value: string; onChange: (value: string) => void }) => <textarea aria-label={label} value={value} onChange={(event) => onChange(event.target.value)} />,
  PairEditor: ({ rows, onChange }: { rows: Pair[]; onChange: (rows: Pair[]) => void }) => <>{rows.map((row) => <input key={row.id} aria-label={row.key} value={row.value} onChange={(event) => onChange(rows.map((item) => item.id === row.id ? { ...item, value: event.target.value } : item))} />)}</>,
}));
let data: WorkspaceData;
beforeEach(async () => { await setLanguage("en"); vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} }); data = initialData(); data.collections[0].id = "root"; });
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });

it("keeps current requests when Apply changes only collection settings", () => {
  const apply = vi.fn();
  const props = { target: { kind: "edit" as const, id: "root" }, dark: false, onApply: apply, close: vi.fn() };
  const page = render(<Theme><SharedCollectionSettingsDialog {...props} data={data} /></Theme>);
  fireEvent.change(screen.getByRole("textbox", { name: "Name" }), { target: { value: "Settings" } });
  const current = structuredClone(data); current.collections[0].requests[0].name = "Concurrent request";
  page.rerender(<Theme><SharedCollectionSettingsDialog {...props} data={current} /></Theme>);
  fireEvent.click(screen.getByRole("button", { name: "Apply" }));
  expect(apply.mock.lastCall?.[0].collections[0].name).toBe("Settings");
  expect(apply.mock.lastCall?.[0].collections[0].requests).toEqual(current.collections[0].requests);
});

it("strips member local overrides and preserves shared variable settings", () => {
  data.collections[0].variables = [{ id: "v", key: "folder", value: "shared", local_value: "member-only", secret: true, enabled: true }];
  const apply = vi.fn();
  render(<Theme><SharedCollectionSettingsDialog target={{ kind: "edit", id: "root" }} data={data} dark={false} onApply={apply} close={vi.fn()} /></Theme>);
  fireEvent.mouseDown(screen.getByRole("tab", { name: /Variables/ }), { button: 0, ctrlKey: false });
  fireEvent.change(screen.getByLabelText("folder"), { target: { value: "updated" } });
  fireEvent.click(screen.getByRole("checkbox", { name: "Use folder variables during execution" }));
  expect(apply).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Apply" }));
  expect(apply.mock.lastCall?.[0].collections[0]).toMatchObject({ variables_enabled: false, variables: [{ value: "updated", secret: true }] });
  expect(apply.mock.lastCall?.[0].collections[0].variables[0].local_value).toBeUndefined();
});

it.each(["oauth1", "oauth2"] as const)("edits %s configuration without mounting an owned token manager", (kind) => {
  data.collections[0].auth = { kind, token: "", username: "", password: "" };
  render(<Theme><SharedCollectionSettingsDialog target={{ kind: "edit", id: "root" }} data={data} dark={false} onApply={vi.fn()} close={vi.fn()} /></Theme>);
  expect(screen.getByRole("dialog")).toBeTruthy();
  expect(screen.queryByText(/token manager/i)).toBeNull();
});

it("excludes self and descendants as parents and rejects concurrent metadata changes", () => {
  data.collections.push({ id: "child", parent_id: "root", name: "Child", description: "", requests: [] });
  const apply = vi.fn();
  const props = { target: { kind: "edit" as const, id: "root" }, dark: false, onApply: apply, close: vi.fn() };
  const page = render(<Theme><SharedCollectionSettingsDialog {...props} data={data} /></Theme>);
  expect(screen.queryByRole("option", { name: "Child" })).toBeNull();
  const current = structuredClone(data); current.collections[0].name = "Remote metadata";
  page.rerender(<Theme><SharedCollectionSettingsDialog {...props} data={current} /></Theme>);
  fireEvent.click(screen.getByRole("button", { name: "Apply" }));
  expect(screen.getByRole("alert").textContent).toContain("changed while editing");
  expect(apply).not.toHaveBeenCalled();
});
