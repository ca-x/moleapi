// @vitest-environment jsdom
import {
  fireEvent,
  render,
  screen,
  act,
  cleanup,
} from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { api } from "../../shared/api";
import { useWorkbench } from "../workbench/context";
import SoapSourceDialog from "./SoapSourceDialog";
import type { useSoapSchema } from "./useSoapSchema";
vi.mock("../../shared/api", () => ({ api: vi.fn(), native: false }));
vi.mock("../workbench/context", () => ({ useWorkbench: vi.fn() }));
vi.mock("../../shared/ui", () => ({
  Editor: ({
    value,
    onChange,
    label,
    readOnly,
  }: {
    readOnly?: boolean;
    value: string;
    onChange: (value: string) => void;
    label: string;
  }) => (
    <textarea
      aria-label={label}
      readOnly={readOnly}
      value={value}
      onChange={(event) => onChange(event.target.value)}
    />
  ),
  Field: ({
    label,
    children,
  }: {
    label: string;
    children: React.ReactNode;
  }) => (
    <label>
      {label}
      {children}
    </label>
  ),
  Choice: ({ value, label, options, onChange, disabled }: {
    value: string; label: string; options: { value: string; label: string }[];
    onChange: (value: string) => void; disabled?: boolean;
  }) => <select aria-label={label} value={value} disabled={disabled} onChange={event => onChange(event.target.value)}>
    {options.map(option => <option key={option.value} value={option.value}>{option.label}</option>)}
  </select>,
  ToolButton: ({
    label,
    onClick,
    children,
    disabled,
  }: {
    disabled?: boolean;
    label: string;
    onClick: () => void;
    children: React.ReactNode;
  }) => (
    <button aria-label={label} disabled={disabled} onClick={onClick}>
      {children}
    </button>
  ),
}));
afterEach(cleanup);
it("discarded imports cannot attach a source after closing and reopening the dialog", async () => {
  vi.mocked(useWorkbench).mockReturnValue({
    draft: { id: "workspace", data: { collections: [], active_environment_id: "dev" } },
    localVariables: { values: () => [{ key: "token", value: "private", enabled: true }] },
    dark: false,
  } as unknown as ReturnType<typeof useWorkbench>);
  const attach = vi.fn();
  const source = { guard: () => () => true, attach } as unknown as ReturnType<
    typeof useSoapSchema
  >;
  let resolve!: (value: unknown) => void;
  vi.mocked(api).mockImplementation(
    () =>
      new Promise((done) => {
        resolve = done;
      }),
  );
  const onOpenChange = vi.fn();
  const { rerender } = render(
    <SoapSourceDialog open onOpenChange={onOpenChange} source={source} />,
  );
  fireEvent.click(screen.getByRole("button", { name: "添加文件" }));
  fireEvent.click(screen.getByRole("button", { name: "验证并使用定义" }));
  expect(api).toHaveBeenCalledWith(
    "/api/soap/import",
    "POST",
    expect.objectContaining({ workspace_id: "workspace", environment_id: "dev", locals: [{ key: "token", value: "private", enabled: true }] }),
  );
  rerender(
    <SoapSourceDialog
      open={false}
      onOpenChange={onOpenChange}
      source={source}
    />,
  );
  rerender(
    <SoapSourceDialog open onOpenChange={onOpenChange} source={source} />,
  );
  await act(async () => {
    resolve({ specification: { id: "late" }, schema: { services: [] } });
  });
  expect(attach).not.toHaveBeenCalled();
  expect(onOpenChange).not.toHaveBeenCalledWith(false);
});

it("WSDL validation locks editable fields so later edits cannot be discarded by a stale validated snapshot", async () => {
  vi.mocked(useWorkbench).mockReturnValue({
    draft: { id: "workspace", data: { collections: [], active_environment_id: "dev" } },
    localVariables: { values: () => [{ key: "token", value: "private", enabled: true }] },
    dark: false,
  } as unknown as ReturnType<typeof useWorkbench>);
  const source = {
    guard: () => () => true,
    attach: vi.fn(),
  } as unknown as ReturnType<typeof useSoapSchema>;
  let resolve!: (value: unknown) => void;
  vi.mocked(api).mockImplementation(
    () =>
      new Promise((done) => {
        resolve = done;
      }),
  );
  render(<SoapSourceDialog open onOpenChange={vi.fn()} source={source} />);
  fireEvent.click(screen.getByRole("button", { name: "添加文件" }));
  fireEvent.click(screen.getByRole("button", { name: "验证并使用定义" }));
  expect(screen.getByLabelText("WSDL 名称").hasAttribute("disabled")).toBe(true);
  expect(screen.getByLabelText("WSDL / XSD 相对路径").hasAttribute("disabled")).toBe(
    true,
  );
  expect(
    screen.getByRole("button", { name: "添加文件" }).hasAttribute("disabled"),
  ).toBe(true);
  expect(
    screen.getByRole("button", { name: "移除定义文件" }).hasAttribute("disabled"),
  ).toBe(true);
  expect(
    (screen.getByLabelText("WSDL / XSD 文件内容") as HTMLTextAreaElement).readOnly,
  ).toBe(true);
  await act(async () => {
    resolve({ specification: { id: "valid" }, schema: { services: [] } });
  });
  expect(source.attach).toHaveBeenCalled();
});
