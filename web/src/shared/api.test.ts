// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { api, ApiError, previewMock } from "./api";
afterEach(() => {
  sessionStorage.clear();
  vi.unstubAllGlobals();
});
it("previews an intentional 404 Mock body while rejecting an expired session",async()=>{
  sessionStorage.setItem("moleapi_token","owner");
  const fetcher=vi.fn().mockResolvedValueOnce({status:404,text:async()=>"saved example not found"}).mockResolvedValueOnce({status:401,text:async()=>"expired"});
  vi.stubGlobal("fetch",fetcher);
  expect(await previewMock("/api/mock/w/r/e")).toEqual({status:404,body:"saved example not found"});
  expect(fetcher).toHaveBeenCalledWith("/api/mock/w/r/e",{headers:{Authorization:"Bearer owner"}});
  await expect(previewMock("/api/mock/w/r/e")).rejects.toBeInstanceOf(ApiError);
});
describe("hosted session expiry", () => {
  it("reports current-session expiry so the app can show login while retaining its draft", async () => {
    sessionStorage.setItem("moleapi_token", "expired");
    vi.stubGlobal(
      "fetch",
      vi
        .fn()
        .mockResolvedValue({
          status: 401,
          json: async () => ({ error: "Expired" }),
        }),
    );
    const expired = vi.fn();
    window.addEventListener("moleapi:unauthorized", expired);
    try {
      await expect(api("/api/workspaces")).rejects.toBeInstanceOf(ApiError);
      expect(expired).toHaveBeenCalledTimes(1);
    } finally {
      window.removeEventListener("moleapi:unauthorized", expired);
    }
  });
  it("does not log out a new session when an older request returns a delayed 401", async () => {
    sessionStorage.setItem("moleapi_token", "old-session");
    let resolve!: (reply: unknown) => void;
    vi.stubGlobal(
      "fetch",
      vi.fn(
        () =>
          new Promise((yes) => {
            resolve = yes;
          }),
      ),
    );
    const expired = vi.fn();
    window.addEventListener("moleapi:unauthorized", expired);
    const pending = api("/api/workspaces");
    sessionStorage.setItem("moleapi_token", "new-session");
    resolve({
      status: 401,
      json: async () => ({ error: "Old session expired" }),
    });
    try {
      await expect(pending).rejects.toBeInstanceOf(ApiError);
      expect(expired).not.toHaveBeenCalled();
    } finally {
      window.removeEventListener("moleapi:unauthorized", expired);
    }
  });
});
