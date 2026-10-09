// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen,waitFor,act} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import ProjectGenerationDialog from "./ProjectGenerationDialog";
const state=vi.hoisted(()=>({authenticated:true,accountId:"owner",draft:{id:"w",data:{specifications:[{id:"s",name:"Fixture",kind:"openapi",source:"spec",dialect:"3.0.3"}]}},dark:false,dirty:false,save:vi.fn()}));
const call=vi.hoisted(()=>vi.fn());
vi.mock("../workbench/context",()=>({useWorkbench:()=>state}));vi.mock("../../shared/api",()=>({api:call,native:false}));
vi.mock("../../shared/ui",async()=>{const real=await vi.importActual<object>("../../shared/ui");return {...real,Editor:({value}:{value:string})=><textarea aria-label="project-code" value={value} readOnly/>};});
beforeEach(async()=>{await setLanguage("en");vi.stubGlobal("ResizeObserver",class{observe(){}unobserve(){}disconnect(){}});state.accountId="owner";state.dirty=false;call.mockReset();call.mockImplementation((path:string)=>path.endsWith("catalog")?Promise.resolve({java_available:false,targets:[{id:"rust-progenitor",kind:"client",upstream_stability:"stable",validation:"fixture",options:{packageName:"sdk"}},{id:"go-server",kind:"server",upstream_stability:"stable",options:{}}]}):Promise.resolve({engine:"progenitor",files:[{path:"src/lib.rs",encoding:"utf8",content:"generated-client",sha256:"hash",bytes:16,executable:false}],archive_base64:"AA==",source_sha256:"hash"}));});
afterEach(()=>{cleanup();vi.unstubAllGlobals();});
it("shows real project files and switches language without changing generation state",async()=>{
 render(<Theme><ProjectGenerationDialog open specificationId="s" onOpenChange={()=>{}}/></Theme>);await waitFor(()=>expect(screen.getByRole("button",{name:"Generate project"}).getAttribute("disabled")).toBeNull());fireEvent.click(screen.getByRole("button",{name:"Generate project"}));await screen.findByDisplayValue("generated-client");expect(call).toHaveBeenCalledWith("/api/generation/projects","POST",expect.objectContaining({workspace_id:"w",specification_id:"s",target:"rust-progenitor",include_secrets:false}));
 await act(()=>setLanguage("zh-CN"));expect(screen.getByRole("button",{name:"下载项目 ZIP"})).toBeTruthy();expect(screen.getByDisplayValue("generated-client")).toBeTruthy();
});
it("close and reopen discard a late project reply even when source and target are identical",async()=>{
 let finish!:(value:unknown)=>void;call.mockImplementation((path:string)=>path.endsWith("catalog")?Promise.resolve({java_available:false,targets:[{id:"rust-progenitor",kind:"client",upstream_stability:"stable",options:{}}]}):path.endsWith("cancel")?Promise.resolve({cancelled:true}):new Promise(resolve=>{finish=resolve;}));
 const view=render(<Theme><ProjectGenerationDialog open specificationId="s" onOpenChange={()=>{}}/></Theme>);await waitFor(()=>expect(screen.getByRole("button",{name:"Generate project"}).getAttribute("disabled")).toBeNull());fireEvent.click(screen.getByRole("button",{name:"Generate project"}));await waitFor(()=>expect(finish).toBeTypeOf("function"));
 view.rerender(<Theme><ProjectGenerationDialog open={false} specificationId="s" onOpenChange={()=>{}}/></Theme>);view.rerender(<Theme><ProjectGenerationDialog open specificationId="s" onOpenChange={()=>{}}/></Theme>);
 await act(async()=>finish({engine:"old",source_sha256:"old",archive_base64:"AA==",files:[{path:"old.rs",encoding:"utf8",content:"old-private-project",sha256:"h",bytes:19}]}));expect(screen.queryByDisplayValue("old-private-project")).toBeNull();expect(call.mock.calls.some(([path])=>path==="/api/generation/projects/cancel")).toBe(true);
});
