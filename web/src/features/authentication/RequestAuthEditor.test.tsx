// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { Theme } from "@radix-ui/themes";
import { useState } from "react";
import { afterEach, expect, it, vi } from "vitest";
import type { Auth } from "../../shared/types";
import { newRequest } from "../../shared/model";
import { setLanguage } from "../../shared/i18n";
import RequestAuthEditor, { apiKeyConfig } from "./RequestAuthEditor";
afterEach(() => { cleanup(); vi.unstubAllGlobals(); vi.restoreAllMocks(); });
it("preserves an API key draft while switching to usable Digest credentials", () => {
  vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} });
  Object.defineProperty(HTMLElement.prototype,"scrollIntoView",{configurable:true,value:vi.fn()});
  setLanguage("en");
  function Host() {
    const [auth, change] = useState<Auth>({...newRequest("auth", "https://example.test").auth,kind:"apikey",api_key:{...apiKeyConfig(),value:"private-draft"}});
    return <Theme><RequestAuthEditor auth={auth} change={change} dark={false}/><output data-testid="auth">{JSON.stringify(auth)}</output></Theme>;
  }
  render(<Host/>);
  fireEvent.keyDown(screen.getByRole("combobox",{name:"Authentication type"}),{key:"Enter"});
  fireEvent.click(screen.getByRole("option",{name:"Digest Auth"}));
  const current=JSON.parse(screen.getByTestId("auth").textContent!);
  expect(current.kind).toBe("digest");
  expect(current.api_key.value).toBe("private-draft");
  expect(screen.getByRole("textbox",{name:"Username"})).toBeTruthy();
});
