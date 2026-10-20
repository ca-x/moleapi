// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { PropsWithChildren } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { api } from "../../shared/api";
import { useAuth } from "./useAuth";

vi.mock("../../shared/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../shared/api")>()),
  api: vi.fn(),
}));

const callsKey = ["shared-mock-calls", "member", "owner", "workspace", 0];

function harness() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  client.setQueryData(callsKey, [{ id: "private-call" }]);
  const wrapper = ({ children }: PropsWithChildren) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  return { client, wrapper };
}

beforeEach(() => {
  sessionStorage.clear();
  sessionStorage.setItem("moleapi_token", "member-token");
  vi.mocked(api).mockReset();
  vi.mocked(api).mockResolvedValue({ mode: "server", setup_required: false, registration_enabled: true });
});

afterEach(() => { cleanup(); sessionStorage.clear(); });

it("removes private workspace history from memory when the session expires", () => {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  client.setQueryData(["workspace-versions", "first-account", "workspace-a"], [{ name: "private history" }]);
  const wrapper = ({ children }: PropsWithChildren) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  renderHook(() => useAuth(), { wrapper });
  act(() => window.dispatchEvent(new Event("moleapi:unauthorized")));
  expect(client.getQueryData(["workspace-versions", "first-account", "workspace-a"])).toBeUndefined();
});

it("drops cached shared Mock calls when the member logs out", async () => {
  const { client, wrapper } = harness();
  const { result } = renderHook(() => useAuth(), { wrapper });
  await act(async () => result.current.endSession());
  expect(client.getQueryData(callsKey)).toBeUndefined();
});

it("drops cached shared Mock calls when the session expires", async () => {
  const { client, wrapper } = harness();
  renderHook(() => useAuth(), { wrapper });
  act(() => window.dispatchEvent(new Event("moleapi:unauthorized")));
  await waitFor(() => expect(client.getQueryData(callsKey)).toBeUndefined());
});
