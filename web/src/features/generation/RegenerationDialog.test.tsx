// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import RegenerationDialog from "./RegenerationDialog";
const call=vi.hoisted(()=>vi.fn());
const pick=vi.hoisted(()=>vi.fn());
vi.mock("../../shared/api",()=>({api:call,pickFile:pick,native:false}));vi.mock("../workbench/context",()=>({useWorkbench:()=>({accountId:"owner",draft:{id:"w"},dark:false})}));
vi.mock("../../shared/ui",async()=>{const real=await vi.importActual<object>("../../shared/ui");return {...real,Editor:({value}:{value:string})=><textarea aria-label="merge-code" readOnly value={value}/>};});
afterEach(()=>{cleanup();vi.clearAllMocks();vi.unstubAllGlobals();});
it("uses prior/current snapshots and requires explicit conflict resolution before archive download",async()=>{
 await setLanguage("en");vi.stubGlobal("ResizeObserver",class{observe(){}unobserve(){}disconnect(){}});
 const f={path:"a.txt",encoding:"utf8" as const,content:"old",bytes:3,sha256:"hash",executable:false};pick.mockResolvedValue(JSON.stringify({files:[f]}));
 call.mockImplementation((_path:string,_method:string,input:unknown)=>Promise.resolve((input as {resolutions?:Record<string,string>}).resolutions?.["a.txt"]?{files:[{path:"a.txt",status:"resolved",encoding:"utf8",content:"old",patch:null}],conflicts:0,archive_base64:"AA=="}:{files:[{path:"a.txt",status:"conflict",encoding:"utf8",content:"markers",patch:"conflict patch"}],conflicts:1,archive_base64:null}));
 const next={engine:"fixture",target:"rust-progenitor",source_sha256:"next",options:{},include_secrets:false,files:[{...f,content:"new"}],archive_base64:"AA=="};
 render(<Theme><RegenerationDialog next={next} open onOpenChange={()=>{}}/></Theme>);fireEvent.click(screen.getByRole("button",{name:"Load previous generation snapshot JSON"}));await waitFor(()=>expect(screen.getByRole("button",{name:"Compare and merge"}).getAttribute("disabled")).toBeNull());fireEvent.click(screen.getByRole("button",{name:"Compare and merge"}));await screen.findByDisplayValue("conflict patch");expect(screen.getByRole("button",{name:"Download merged project ZIP"}).getAttribute("disabled")).not.toBeNull();
 fireEvent.click(screen.getByRole("button",{name:"Keep current edited version"}));fireEvent.click(screen.getByRole("button",{name:"Compare and merge"}));await waitFor(()=>expect(screen.getByRole("button",{name:"Download merged project ZIP"}).getAttribute("disabled")).toBeNull());expect(call).toHaveBeenLastCalledWith("/api/generation/projects/regenerate","POST",expect.objectContaining({resolutions:{"a.txt":"working"},workspace_id:"w"}));
});
