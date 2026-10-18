// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
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
  const data={schema_version:1 as const,collections:[{id:"c",name:"Folder",description:"",requests:[]}],environments:[],active_environment_id:null,documents:[{id:"d",name:"Guide",collection_id:"c",source,secret:false}]};
  const updateData=vi.fn();
  const save=vi.fn();
  const setGuard=vi.fn();
  const context={accountId:"owner",draft:{id:"w",name:"W",revision:1,data},updateData,save,setGuard,dirty:true,saving:false,dark:false};
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
