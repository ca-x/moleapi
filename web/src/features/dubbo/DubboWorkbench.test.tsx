// @vitest-environment jsdom
import { fireEvent, render, screen } from "@testing-library/react";
import { Theme } from "@radix-ui/themes";
import { expect, it, vi } from "vitest";
import DubboWorkbench from "./DubboWorkbench";
import { dubboConfig } from "./model";
import { setLanguage } from "../../shared/i18n";
import { newRequest } from "../../shared/model";
import type { DubboConfig, RequestSpec } from "../../shared/types";
vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} });

it.each(["nacos", "zookeeper"] as const)("clearing %s credentials restores anonymous source", async source => {
  await setLanguage("en");
  const update = vi.fn();
  const request: RequestSpec = {
    ...newRequest("Registry", `${source}://localhost:2181`),
    protocol: {...dubboConfig(),source,source_version:2,
      registry_auth:{username:"reader",password:"private"}},
  };
  const view = render(<Theme><DubboWorkbench request={request} update={update} disabled={false}/></Theme>);
  const label = source === "nacos" ? "Nacos" : "ZooKeeper";
  fireEvent.change(screen.getByLabelText(`${label} username`), {target:{value:""}});
  const first = update.mock.lastCall![0].protocol as DubboConfig;
  expect(first.registry_auth).toEqual({username:"",password:"private"});
  view.rerender(<Theme><DubboWorkbench request={{...request,protocol:first}} update={update} disabled={false}/></Theme>);
  fireEvent.change(screen.getByLabelText(`${label} password`), {target:{value:""}});
  const restored = update.mock.lastCall![0].protocol as DubboConfig;
  expect(restored.registry_auth).toBeUndefined();
  view.unmount();
});
