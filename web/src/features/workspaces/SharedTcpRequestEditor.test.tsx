// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { Theme } from "@radix-ui/themes";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { newRequest } from "../../shared/model";
import { tcpConfig } from "../tcp/model";
import type { RequestSpec } from "../../shared/types";
import type { TcpMessage } from "../tcp/types";
import { setLanguage } from "../../shared/i18n";
import SharedTcpRequestEditor from "./SharedTcpRequestEditor";
const callbacks = vi.hoisted(() => ({ settings: null as null | ((patch: Partial<RequestSpec>) => void), message: null as null | ((message: TcpMessage) => void) }));
vi.mock("../tcp/TcpConnectionSettings", () => ({ default: ({ onChange }: { onChange: (patch: Partial<RequestSpec>) => void }) => { callbacks.settings = onChange; return <div>settings</div>; } }));
vi.mock("../tcp/TcpMessageEditor", () => ({ default: ({ onChange }: { onChange: (message: TcpMessage) => void }) => { callbacks.message = onChange; return <div>message</div>; } }));
vi.mock("../scripts/RequestScripts", () => ({ default: () => null }));
beforeEach(async () => { await setLanguage("zh-CN"); });
afterEach(cleanup);
it("preserves newer TCP framing when a retained message callback changes the saved template", () => {
  const change = vi.fn(); const request = { ...newRequest(), protocol: tcpConfig() };
  function view(request: RequestSpec) { return <Theme><SharedTcpRequestEditor request={request} scope="same" dark={false} disabled={false} isCurrent={() => true} onChange={change} /></Theme>; }
  const { rerender } = render(view(request));
  fireEvent.mouseDown(screen.getByRole("tab", { name: /已保存报文/ }), { button: 0 });
  const original = callbacks.message!;
  rerender(view({ ...request, protocol: { ...tcpConfig(), framing: "lines" } }));
  act(() => original({ encoding: "text", payload_source: "edited", secret: false }));
  expect(change).toHaveBeenCalledWith({ protocol: { ...tcpConfig(), framing: "lines", message: { encoding: "text", payload_source: "edited", secret: false } } });
});
it("does not apply a retained settings/certificate callback to another scope", () => {
  const change = vi.fn(); const request = { ...newRequest(), protocol: tcpConfig() };
  function view(scope: string) { return <Theme><SharedTcpRequestEditor request={request} scope={scope} dark={false} disabled={false} isCurrent={() => true} onChange={change} /></Theme>; }
  const { rerender } = render(view("old")); const old = callbacks.settings!;
  rerender(view("new")); act(() => old({ network: undefined }));
  expect(change).not.toHaveBeenCalled();
});
