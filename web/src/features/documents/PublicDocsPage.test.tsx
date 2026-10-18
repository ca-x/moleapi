// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import PublicDocsPage from "./PublicDocsPage";

afterEach(()=>{cleanup();vi.unstubAllGlobals();});

it("loads a public snapshot without credentials and renders only safe Markdown",async()=>{
  await setLanguage("en");
  const fetcher=vi.fn().mockResolvedValue({ok:true,json:async()=>({
    title:"Public site",documents:[
      {title:"Guide",folder:"Manuals",source:"# Welcome\n\n<script>alert(1)</script>\n\n![remote](https://evil.test/i.png) [internal](moleapi://request/secret) [safe](https://example.test)"},
      {title:"Second",folder:null,source:"Other page"}
    ]
  })});
  vi.stubGlobal("fetch",fetcher);
  const page=render(<PublicDocsPage id={"a".repeat(32)}/>);
  await waitFor(()=>expect(screen.getByRole("heading",{name:"Welcome"})).toBeTruthy());
  expect(fetcher).toHaveBeenCalledWith(`/api/public/docs/${"a".repeat(32)}`,expect.objectContaining({credentials:"omit",cache:"no-store"}));
  expect(page.container.querySelector("script,img")).toBeNull();
  expect(screen.queryByRole("button",{name:"internal"})).toBeNull();
  expect(screen.queryByRole("link",{name:"internal"})).toBeNull();
  expect(screen.getByRole("link",{name:"safe"}).getAttribute("rel")).toContain("noreferrer");
  fireEvent.change(screen.getByRole("textbox",{name:"Search published docs"}),{target:{value:"Second"}});
  expect(screen.queryByRole("button",{name:"Guide"})).toBeNull();
  expect(screen.getByRole("status").textContent).toContain("1 published documents");
  fireEvent.click(screen.getByRole("button",{name:"Second"}));
  expect(screen.getByText("Other page")).toBeTruthy();
  fireEvent.change(screen.getByRole("textbox",{name:"Search published docs"}),{target:{value:"none"}});
  expect(screen.getByRole("status").textContent).toContain("No published documents");
  expect(screen.getByText("Other page")).toBeTruthy();
  expect(fetcher).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole("button",{name:"Chinese (Simplified)"}));
  await waitFor(()=>expect(screen.getByRole("textbox",{name:"搜索公开文档"})).toBeTruthy());
  await waitFor(()=>expect(document.title).toBe("Public site · MoleAPI"));
});

it("rejects malformed publication IDs before any fetch",async()=>{
  await setLanguage("en");
  const fetcher=vi.fn();
  vi.stubGlobal("fetch",fetcher);
  render(<PublicDocsPage id="bad/id"/>);
  expect(await screen.findByRole("heading",{name:"Documentation unavailable"})).toBeTruthy();
  expect(fetcher).not.toHaveBeenCalled();
});
