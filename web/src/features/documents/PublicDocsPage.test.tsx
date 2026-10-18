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

it("keeps protected source locked until a correct in-memory password is submitted",async()=>{
  await setLanguage("en");
  const id="b".repeat(32);
  const fetcher=vi.fn()
    .mockResolvedValueOnce({ok:false,status:403})
    .mockResolvedValueOnce({ok:false,status:401})
    .mockResolvedValueOnce({ok:true,status:200,json:async()=>({title:"Protected site",documents:[{title:"Guide",folder:null,source:"# Protected source"}]})});
  vi.stubGlobal("fetch",fetcher);
  const page=render(<PublicDocsPage id={id}/>);
  await screen.findByRole("heading",{name:"Password-protected documentation"});
  expect(screen.queryByText("Protected site")).toBeNull();
  expect(screen.queryByText("Protected source")).toBeNull();
  fireEvent.change(screen.getByLabelText("Access password"),{target:{value:"wrong-password"}});
  fireEvent.click(screen.getByRole("button",{name:"Unlock documents"}));
  await screen.findByRole("alert");
  expect(screen.getByRole("alert").textContent).toContain("Incorrect password");
  expect(screen.getByLabelText("Access password")).toHaveProperty("value","");
  fireEvent.change(screen.getByLabelText("Access password"),{target:{value:"correct-password-123"}});
  fireEvent.click(screen.getByRole("button",{name:"Unlock documents"}));
  await screen.findByRole("heading",{name:"Protected source"});
  expect(fetcher).toHaveBeenCalledWith(`/api/public/docs/${id}/unlock`,expect.objectContaining({method:"POST",credentials:"omit",cache:"no-store"}));
  expect(fetcher).toHaveBeenCalledTimes(3);
  expect(page.container.querySelector("input[type=password]")).toBeNull();
  page.unmount();
  fetcher.mockResolvedValueOnce({ok:false,status:403});
  render(<PublicDocsPage id={id}/>);
  expect(await screen.findByRole("heading",{name:"Password-protected documentation"})).toBeTruthy();
});

it("shows a bounded retry message after password rate limiting",async()=>{
  await setLanguage("en");
  const fetcher=vi.fn()
    .mockResolvedValueOnce({ok:false,status:403})
    .mockResolvedValueOnce({ok:false,status:429,headers:{get:()=>"42"}});
  vi.stubGlobal("fetch",fetcher);
  render(<PublicDocsPage id={"c".repeat(32)}/>);
  await screen.findByRole("heading",{name:"Password-protected documentation"});
  fireEvent.change(screen.getByLabelText("Access password"),{target:{value:"wrong-password"}});
  fireEvent.click(screen.getByRole("button",{name:"Unlock documents"}));
  expect((await screen.findByRole("alert")).textContent).toContain("42 seconds");
});
