// @vitest-environment jsdom
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { useWorkbench } from "../workbench/context";
import SocketIoWorkbench from "./SocketIoWorkbench";
import type { SocketIoConfig } from "../../shared/types";
vi.mock("../workbench/context", () => ({ useWorkbench: vi.fn() }));
vi.mock("../../shared/ui", () => ({
  Editor: ({
    value,
    onChange,
    label,
    readOnly,
  }: {
    value: string;
    onChange?: (value: string) => void;
    label: string;
    readOnly?: boolean;
  }) => (
    <textarea
      aria-label={label}
      value={value}
      readOnly={readOnly}
      onChange={(event) => onChange?.(event.target.value)}
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
  ToolButton: ({
    label,
    onClick,
    disabled,
  }: {
    label: string;
    onClick: () => void;
    disabled?: boolean;
  }) => <button aria-label={label} disabled={disabled} onClick={onClick} />,
}));
vi.mock("../protocols/SessionEventPane", () => ({
  default: ({
    onReply,
    canReply,
  }: {
    onReply?: (event: unknown) => void;
    canReply?: (event: unknown) => boolean;
  }) => {
    const event = {
      cursor: 1,
      direction: "incoming",
      message: {
        kind: "socketio_event",
        event: "question",
        arguments: [],
        attachments_base64: [],
        ack_id: "reply-token",
      },
    };
    return (
      <button
        disabled={!onReply || (canReply ? !canReply(event) : false)}
        onClick={() => onReply?.(event)}
      >
        确认回调
      </button>
    );
  },
}));
afterEach(cleanup);
it("a listener update preserves newer saved payload edits made during its async command", async () => {
  const original: SocketIoConfig = {
    kind: "socketio",
    namespace: "/",
    path: "/socket.io/",
    auth_source: "{}",
    listeners: ["message"],
    event: "echo",
    arguments_source: "[]",
    attachments_base64: [],
    request_ack: false,
    ack_timeout_ms: 5000,
  };
  let config = original;
  const updateRequest = vi.fn();
  let resolve!: (value: boolean) => void;
  const send = vi.fn(
    () =>
      new Promise<boolean>((done) => {
        resolve = done;
      }),
  );
  const state = {
    authenticated: true,
    accountId: "account",
    draft: { id: "workspace", data: { active_environment_id: "dev" } },
    request: { id: "request", protocol: config },
    dark: false,
    updateRequest,
    protocolSession: {
      session: { id: "live", state: "open", received_bytes: 0, sent_bytes: 0 },
      events: [],
      dropped: 0,
      error: "",
      busy: false,
      sending: false,
      send,
      close: vi.fn(),
    },
  };
  vi.mocked(useWorkbench).mockImplementation(
    () => state as unknown as ReturnType<typeof useWorkbench>,
  );
  const { rerender } = render(<SocketIoWorkbench />);
  fireEvent.mouseDown(screen.getByRole("tab", { name: /监听事件/ }), {
    button: 0,
    ctrlKey: false,
  });
  fireEvent.change(screen.getByLabelText("监听事件名称"), {
    target: { value: "echo" },
  });
  fireEvent.click(screen.getByRole("button", { name: "添加监听" }));
  expect(send).toHaveBeenCalledWith({
    kind: "socketio_listen",
    event: "echo",
    enabled: true,
  });
  config = { ...original, arguments_source: '[{"latest":true}]' };
  state.request.protocol = config;
  rerender(<SocketIoWorkbench />);
  await act(async () => {
    resolve(true);
  });
  expect(updateRequest).toHaveBeenCalledWith({
    protocol: { ...config, listeners: ["message", "echo"] },
  });
});

it("a successfully submitted ACK remains consumed when its dialog was cancelled while sending", async () => {
  const config: SocketIoConfig = {
    kind: "socketio",
    namespace: "/",
    path: "/socket.io/",
    auth_source: "{}",
    listeners: ["question"],
    event: "echo",
    arguments_source: "[]",
    attachments_base64: [],
    request_ack: false,
    ack_timeout_ms: 5000,
  };
  let resolve!: (value: boolean) => void;
  const state = {
    authenticated: true,
    accountId: "account",
    draft: { id: "workspace", data: { active_environment_id: "dev" } },
    request: { id: "request", protocol: config },
    dark: false,
    updateRequest: vi.fn(),
    protocolSession: {
      session: {
        id: "session",
        state: "open",
        received_bytes: 0,
        sent_bytes: 0,
      },
      events: [],
      dropped: 0,
      error: "",
      busy: false,
      sending: false,
      send: vi.fn(
        () =>
          new Promise<boolean>((done) => {
            resolve = done;
          }),
      ),
      close: vi.fn(),
    },
  };
  vi.mocked(useWorkbench).mockReturnValue(
    state as unknown as ReturnType<typeof useWorkbench>,
  );
  render(<SocketIoWorkbench />);
  fireEvent.mouseDown(screen.getByRole("tab", { name: /事件与 ACK/ }), {
    button: 0,
    ctrlKey: false,
  });
  fireEvent.click(screen.getByRole("button", { name: "确认回调" }));
  fireEvent.click(screen.getByRole("button", { name: "回复 ACK" }));
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  await act(async () => {
    resolve(true);
  });
  expect(
    screen.getByRole("button", { name: "确认回调" }).hasAttribute("disabled"),
  ).toBe(true);
});
