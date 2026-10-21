// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { Theme } from "@radix-ui/themes";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { setLanguage } from "../../shared/i18n";
import type { ProtocolSession } from "../protocols/types";
import type { TcpMessage } from "../tcp/types";
import SharedTcpSessionControls from "./SharedTcpSessionControls";
vi.mock("../tcp/TcpMessageEditor", () => ({ default: ({ message, onChange, children }: { message: TcpMessage; onChange: (message: TcpMessage) => void; children: React.ReactNode }) => <><label>local message<input value={message.payload_source} onChange={(e) => onChange({ ...message, payload_source: e.target.value })} /></label>{children}</> }));
const initial: TcpMessage = { encoding: "text", payload_source: "saved", secret: false };
const session: ProtocolSession = { id: "s", workspace_id: "w", request_id: "r", protocol: "tcp", state: "open", url: "tcp://fixture.test:9000", created_at: "now", updated_at: "now", reason: null, received_bytes: 0, sent_bytes: 0, event_count: 0, handshake: null, client_half_closed: false };
beforeEach(async () => { await setLanguage("zh-CN"); });
afterEach(cleanup);
it("keeps the saved template unchanged and sends only an explicit session-local draft", () => {
  const send = vi.fn().mockResolvedValue(true);
  render(<Theme><SharedTcpSessionControls session={session} initial={initial} dark={false} sending={false} isCurrent={() => true} send={send} /></Theme>);
  fireEvent.change(screen.getByLabelText("local message"), { target: { value: "temporary" } });
  expect(send).not.toHaveBeenCalled(); expect(initial.payload_source).toBe("saved");
  fireEvent.click(screen.getByRole("button", { name: "发送报文" }));
  expect(send).toHaveBeenCalledWith({ kind: "tcp_send", message: { ...initial, payload_source: "temporary" } });
});
it.each(["scope", "session", "closed", "half", "sending"] as const)("does not retarget pending TCP half-close after %s changes", (boundary) => {
  const send = vi.fn().mockResolvedValue(true); let current = true;
  function view(changed = false) { return <Theme><SharedTcpSessionControls session={{ ...session, ...(changed && boundary === "session" ? { id: "other" } : {}), ...(changed && boundary === "closed" ? { state: "closed" as const } : {}), client_half_closed: changed && boundary === "half" }} initial={initial} dark={false} sending={changed && boundary === "sending"} isCurrent={() => current} send={send} /></Theme>; }
  const { rerender } = render(view());
  fireEvent.click(screen.getByRole("button", { name: "半关闭" }));
  if (boundary === "scope") current = false;
  rerender(view(true));
  fireEvent.click(screen.getByRole("button", { name: "确认半关闭" }));
  expect(send).not.toHaveBeenCalled();
});
it("cancels half-close without sending EOF", () => {
  const send = vi.fn().mockResolvedValue(true);
  render(<Theme><SharedTcpSessionControls session={session} initial={initial} dark={false} sending={false} isCurrent={() => true} send={send} /></Theme>);
  fireEvent.click(screen.getByRole("button", { name: "半关闭" })); fireEvent.click(screen.getByRole("button", { name: "取消" }));
  expect(send).not.toHaveBeenCalled();
});
