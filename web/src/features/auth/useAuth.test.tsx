// @vitest-environment jsdom
import { act, cleanup, renderHook } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { PropsWithChildren } from "react";
import { afterEach, expect, it, vi } from "vitest";
import { api } from "../../shared/api";
import { useAuth } from "./useAuth";

vi.mock("../../shared/api", async (original) => ({
  ...(await original<typeof import("../../shared/api")>()),
  api: vi.fn().mockResolvedValue({ mode: "server", setup_required: false, registration_enabled: true }),
}));
afterEach(() => { cleanup(); sessionStorage.clear(); vi.mocked(api).mockReset(); });

it("removes private workspace history from memory when the session expires", () => {
  sessionStorage.setItem("moleapi_token", "test-session");
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  client.setQueryData(["workspace-versions", "first-account", "workspace-a"], [{ name: "private history" }]);
  const wrapper = ({ children }: PropsWithChildren) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  renderHook(() => useAuth(), { wrapper });
  act(() => window.dispatchEvent(new Event("moleapi:unauthorized")));
  expect(client.getQueryData(["workspace-versions", "first-account", "workspace-a"])).toBeUndefined();
});
