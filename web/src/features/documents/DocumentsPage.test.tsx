// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import {newRequest} from "../../shared/model";
import type {WorkspaceData} from "../../shared/types";
import {useWorkbench} from "../workbench/context";
import DocumentsPage from "./DocumentsPage";

vi.mock("../workbench/context",()=>({useWorkbench:vi.fn()}));
vi.mock("../../shared/ui",async original=>({
  ...await original<typeof import("../../shared/ui")>(),
  Editor:({value,onChange,label}:{value:string;onChange?:(source:string)=>void;label:string})=><textarea aria-label={label} value={value} onChange={event=>onChange?.(event.target.value)}/>
}));
beforeEach(()=>vi.stubGlobal("ResizeObserver",class {observe(){} unobserve(){} disconnect(){}}));
afterEach(()=>{cleanup();vi.clearAllMocks();});

function setup(source="# Guide\n\n[Safe](https://example.test) [Unsafe](javascript:alert(1)) ![Remote](https://evil.test/x.png)\n\n<script>alert(1)</script>") {
  const data:WorkspaceData={schema_version:1,collections:[{id:"c",name:"Folder",description:"",requests:[]}],environments:[],active_environment_id:null,documents:[{id:"d",name:"Guide",collection_id:"c",source,secret:false}]};
  const updateData=vi.fn();
  const save=vi.fn();
  const setGuard=vi.fn();
  const draft={id:"w",name:"W",revision:1,data};
  const context={accountId:"owner",draft,stateRef:{current:{draft}},accountRef:{current:"owner"},setRequestId:vi.fn(),setView:vi.fn(),setSidebar:vi.fn(),updateData,save,setGuard,dirty:true,saving:false,dark:false};
  vi.mocked(useWorkbench).mockReturnValue(context as never);
  return {data,updateData,save,setGuard,context};
}

it("renders Markdown without executable HTML, automatic images or unsafe links",async()=>{
  await setLanguage("en");
  setup();
  const page=render(<Theme><DocumentsPage/></Theme>);
  expect(page.container.querySelector(".markdown-preview h1")?.textContent).toBe("Guide");
  const safe=screen.getByRole("link",{name:"Safe"});
  expect(safe.getAttribute("href")).toBe("https://example.test/");
  expect(safe.getAttribute("rel")).toContain("noopener");
  expect(page.container.querySelector("script")).toBeNull();
  expect(page.container.querySelector("img")).toBeNull();
  expect(page.container.querySelector('a[href^="javascript:"]')).toBeNull();
  expect(screen.getByText("Remote")).toBeTruthy();
});

it("creates private documents and edits saved drafts through the workspace controller",async()=>{
  await setLanguage("en");
  const {data,updateData,save,context}=setup("# Guide");
  const page=render(<Theme><DocumentsPage/></Theme>);
  fireEvent.click(screen.getByRole("button",{name:"New document"}));
  const created=updateData.mock.lastCall?.[0](data);
  expect(created.documents).toHaveLength(2);
  expect(created.documents[1].secret).toBe(true);
  vi.mocked(useWorkbench).mockReturnValue({...context,draft:{...context.draft,data:created}} as never);
  page.rerender(<Theme><DocumentsPage/></Theme>);
  expect(screen.getByRole("textbox",{name:"Document title"})).toHaveProperty("value","New document");
  fireEvent.change(screen.getByRole("textbox",{name:"Markdown source"}),{target:{value:"# Updated"}});
  const edited=updateData.mock.lastCall?.[0](created);
  expect(edited.documents[1].source).toBe("# Updated");
  fireEvent.click(screen.getByRole("button",{name:"Save workspace"}));
  expect(save).toHaveBeenCalled();
  await setLanguage("zh-CN");
  expect(screen.getByRole("button",{name:"新建文档"})).toBeTruthy();
});

it("requires confirmation before removing a document from the draft",async()=>{
  await setLanguage("en");
  const {data,updateData,setGuard}=setup("# Guide");
  render(<Theme><DocumentsPage/></Theme>);
  fireEvent.click(screen.getByRole("button",{name:"Delete document"}));
  expect(updateData).not.toHaveBeenCalled();
  const guard=setGuard.mock.lastCall?.[0];
  guard.action();
  expect(updateData.mock.lastCall?.[0](data).documents).toEqual([]);
});

it("keeps save disabled when a different document has an invalid title or oversized source",async()=>{
  await setLanguage("en");
  const {data,context,save}=setup("# Guide");
  const invalid={...data,documents:[...(data.documents??[]),{id:"other",name:"Bad ",collection_id:null,source:"x".repeat(256*1024+1),secret:true}]};
  vi.mocked(useWorkbench).mockReturnValue({...context,draft:{...context.draft,data:invalid}} as never);
  render(<Theme><DocumentsPage/></Theme>);
  fireEvent.click(screen.getByRole("button",{name:/Bad/}));
  fireEvent.click(screen.getByRole("button",{name:/Guide/}));
  expect(screen.getByRole("button",{name:"Save workspace"}).hasAttribute("disabled")).toBe(true);
  expect(screen.getByText(/Document “Bad ” needs a 1–128 character title/)).toBeTruthy();
  expect(screen.getByText(/Markdown content in “Bad ” must be at most 256 KiB/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"Save workspace"}));
  expect(save).not.toHaveBeenCalled();
});

it("blocks save when valid documents exceed the aggregate Markdown limit",async()=>{
  await setLanguage("en");
  const {data,context,save}=setup("");
  const largeDocuments=Array.from({length:9},(_,index)=>({id:`large-${index}`,name:`Part ${index}`,collection_id:null,source:"x".repeat(256*1024),secret:true}));
  vi.mocked(useWorkbench).mockReturnValue({...context,draft:{...context.draft,data:{...data,documents:[...(data.documents??[]),...largeDocuments]}}} as never);
  render(<Theme><DocumentsPage/></Theme>);
  expect(screen.getByRole("button",{name:"Save workspace"}).hasAttribute("disabled")).toBe(true);
  expect(screen.getByText("Workspace Markdown content must total at most 2 MiB.")).toBeTruthy();
  expect(save).not.toHaveBeenCalled();
});

it("switches to live generated API docs without changing the workspace draft",async()=>{
  await setLanguage("en");
  const {data,context,updateData}=setup("# Guide");
  const request=newRequest("List pets","https://example.test/pets");
  request.id="pets";
  data.collections[0].requests.push(request);
  const page=render(<Theme><DocumentsPage/></Theme>);
  fireEvent.mouseDown(screen.getByRole("tab",{name:/API reference/}),{button:0});
  expect(screen.getByRole("tab",{name:/API reference/}).getAttribute("aria-selected")).toBe("true");
  expect(screen.getByRole("button",{name:"Save workspace"})).toBeTruthy();
  expect(screen.getByRole("heading",{name:"List pets"})).toBeTruthy();
  expect(updateData).not.toHaveBeenCalled();
  const changed={...data,collections:data.collections.map(collection=>({...collection,requests:collection.requests.map(item=>({...item,name:"List animals"}))}))};
  vi.mocked(useWorkbench).mockReturnValue({...context,draft:{...context.draft,data:changed}} as never);
  page.rerender(<Theme><DocumentsPage/></Theme>);
  expect(screen.getByRole("heading",{name:"List animals"})).toBeTruthy();
  fireEvent.mouseDown(screen.getByRole("tab",{name:/Markdown/}),{button:0});
  expect(screen.getAllByRole("heading",{name:"Guide"}).length).toBeGreaterThan(0);
});

it("opens only a current-workspace request and refuses stale or foreign references",async()=>{
  await setLanguage("en");
  const {data,context,updateData}=setup("[Open](moleapi://request/owned) [Missing](moleapi://request/foreign)");
  const request=newRequest("Owned","https://example.test");
  request.id="owned";
  data.collections[0].requests.push(request);
  render(<Theme><DocumentsPage/></Theme>);
  fireEvent.click(screen.getByRole("button",{name:"Open"}));
  expect(context.setRequestId).toHaveBeenCalledExactlyOnceWith("owned");
  expect(context.setView).toHaveBeenCalledExactlyOnceWith("requests");
  expect(context.setSidebar).toHaveBeenCalledExactlyOnceWith(false);
  expect(screen.queryByRole("button",{name:"Missing"})).toBeNull();
  expect(updateData).not.toHaveBeenCalled();
  vi.clearAllMocks();
  context.stateRef.current.draft={...context.draft,id:"another"};
  fireEvent.click(screen.getByRole("button",{name:"Open"}));
  expect(context.setRequestId).not.toHaveBeenCalled();
  context.stateRef.current.draft=context.draft;
  context.accountRef.current="other";
  fireEvent.click(screen.getByRole("button",{name:"Open"}));
  expect(context.setRequestId).not.toHaveBeenCalled();
});
