// @vitest-environment jsdom
import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import OpenapiSourceBundleDialog,{checkedSourceFiles} from "./OpenapiSourceBundleDialog";
const mock=vi.hoisted(()=>({pick:vi.fn(),api:vi.fn(),update:vi.fn()}));
vi.mock("./projectInputs",async()=>{const real=await vi.importActual<object>("./projectInputs");return {...real,pickProjectSource:mock.pick};});
vi.mock("../../shared/api",()=>({api:mock.api,native:false}));
vi.mock("../workbench/context",()=>({useWorkbench:()=>({accountId:"owner",authenticated:true,draft:{id:"w"},dark:false,updateData:mock.update})}));
vi.mock("../../shared/ui",async()=>{const real=await vi.importActual<object>("../../shared/ui");return {...real,Editor:({value}:{value:string})=><textarea aria-label="source-code" readOnly value={value}/>};});
const content='openapi: 3.0.3\ninfo:\n  title: Bundle\n  version: "1"\npaths: {}\n';
const file={path:"api/openapi.yaml",encoding:"utf8" as const,content,executable:false};
afterEach(()=>{cleanup();vi.clearAllMocks();vi.unstubAllGlobals();});
async function ready(){await setLanguage("en");vi.stubGlobal("ResizeObserver",class{observe(){}unobserve(){}disconnect(){}});}
it("keeps original selected source text in a versioned definition envelope",async()=>{
 await ready();mock.pick.mockResolvedValue({kind:"files",files:[file]});const close=vi.fn();
 render(<Theme><OpenapiSourceBundleDialog open onOpenChange={close}/></Theme>);
 fireEvent.click(screen.getByRole("button",{name:"Load source directory"}));expect((await screen.findByRole("textbox",{name:"source-code"}) as HTMLTextAreaElement).value).toBe(content);
 fireEvent.change(screen.getByRole("textbox",{name:"Definition name"}),{target:{value:"My source files"}});
 fireEvent.click(screen.getByRole("button",{name:"Save multi-file definition"}));
 const update=mock.update.mock.calls[0][0];const spec=update({specifications:[]}).specifications[0];
 expect(spec).toMatchObject({name:"My source files",kind:"openapi",dialect:"3.0.3"});
 expect(JSON.parse(spec.source)).toEqual({format:"moleapi-openapi-source-v1",entry_file:file.path,files:[{path:file.path,content}]});expect(close).toHaveBeenCalledWith(false);
});
it("uses the owned import route for ZIP and discards a result after identical close/reopen",async()=>{
 await ready();mock.pick.mockResolvedValue({kind:"zip",archive_base64:"archive"});let finish!:(value:unknown)=>void;
 mock.api.mockImplementation((path:string)=>path.endsWith("/import")?new Promise(resolve=>{finish=resolve;}):Promise.resolve({}));
 const view=render(<Theme><OpenapiSourceBundleDialog open onOpenChange={()=>{}}/></Theme>);
 fireEvent.click(screen.getByRole("button",{name:"Load source ZIP"}));await waitFor(()=>expect(finish).toBeDefined());
 expect(mock.api).toHaveBeenCalledWith("/api/generation/projects/import","POST",expect.objectContaining({workspace_id:"w",role:"working",source:{kind:"zip",archive_base64:"archive"}}));
 view.rerender(<Theme><OpenapiSourceBundleDialog open={false} onOpenChange={()=>{}}/></Theme>);view.rerender(<Theme><OpenapiSourceBundleDialog open onOpenChange={()=>{}}/></Theme>);
 await act(async()=>finish({files:[file]}));expect(screen.queryByDisplayValue(content)).toBeNull();expect(mock.api.mock.calls.some(([path])=>path.endsWith("/cancel"))).toBe(true);
});
it("filters source types and rejects unsafe paths, invalid UTF8 and oversized bundles",()=>{
 expect(checkedSourceFiles([file,{...file,path:"README.md"}])).toEqual([{path:file.path,content}]);
 for(const files of [[{...file,path:"../outside.yaml"}],[{...file,encoding:"base64" as const,content:"/w=="}],[{...file,content:"x".repeat(1024*1024+1)}]])expect(()=>checkedSourceFiles(files)).toThrow();
});
