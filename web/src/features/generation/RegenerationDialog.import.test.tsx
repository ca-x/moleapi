// @vitest-environment jsdom
import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import RegenerationDialog from "./RegenerationDialog";
const mocks=vi.hoisted(()=>({api:vi.fn(),pick:vi.fn(),source:vi.fn()}));
vi.mock("../../shared/api",()=>({api:mocks.api,pickFile:mocks.pick,native:false}));
vi.mock("./projectInputs",()=>({pickProjectSource:mocks.source}));
vi.mock("../workbench/context",()=>({useWorkbench:()=>({accountId:"owner",draft:{id:"w"},dark:false})}));
vi.mock("../../shared/ui",async()=>{const real=await vi.importActual<object>("../../shared/ui");return {...real,Editor:({value}:{value:string})=><textarea aria-label="merge-code" readOnly value={value}/>,Choice:({label,value,onChange,options}:{label:string;value:string;onChange:(value:string)=>void;options:{value:string;label:string}[]})=><select aria-label={label} value={value} onChange={event=>onChange(event.target.value)}>{options.map(option=><option key={option.value} value={option.value}>{option.label}</option>)}</select>};});
const f={path:"run",encoding:"utf8" as const,content:"old",bytes:3,sha256:"hash",executable:true};
const next={engine:"fixture",target:"rust-progenitor",source_sha256:"next",options:{},include_secrets:false,files:[f],archive_base64:"AA=="};
async function ready(){await setLanguage("en");vi.stubGlobal("ResizeObserver",class{observe(){}unobserve(){}disconnect(){}});}
afterEach(()=>{cleanup();vi.clearAllMocks();vi.unstubAllGlobals();});
it("uses the shared import API for ZIP baselines and directory edits, preserving known browser executable modes",async()=>{
 await ready();mocks.source.mockResolvedValueOnce({kind:"zip",archive_base64:"previous"}).mockResolvedValueOnce({kind:"files",files:[{path:"run",encoding:"utf8",content:"edited",executable:true}]});
 mocks.api.mockResolvedValue({files:[f]});
 render(<Theme><RegenerationDialog next={next} open onOpenChange={()=>{}}/></Theme>);
 fireEvent.click(screen.getByRole("button",{name:"Load previous generated project ZIP"}));await screen.findByText("Previous: 1 files; current: 1 files; new: 1 files.");
 expect(mocks.api).toHaveBeenCalledWith("/api/generation/projects/import","POST",expect.objectContaining({role:"previous",source:{kind:"zip",archive_base64:"previous"},workspace_id:"w"}));
 fireEvent.click(screen.getByRole("button",{name:"Load current project directory"}));
 await waitFor(()=>expect(mocks.source).toHaveBeenLastCalledWith("directory",expect.any(Function),new Map([["run",true]])));
 await waitFor(()=>expect(mocks.api).toHaveBeenLastCalledWith("/api/generation/projects/import","POST",expect.objectContaining({role:"working",source:{kind:"files",files:[{path:"run",encoding:"utf8",content:"edited",executable:true}]}})));
});
it("discards an import completing after close and identical reopen, cancels its lease and remains ready for a new import",async()=>{
 await ready();mocks.source.mockResolvedValue({kind:"zip",archive_base64:"previous"});
 let finish!:(value:unknown)=>void;
 mocks.api.mockImplementation((path:string)=>path.endsWith("/import")?new Promise(resolve=>{finish=resolve;}):Promise.resolve({}));
 const view=render(<Theme><RegenerationDialog next={next} open onOpenChange={()=>{}}/></Theme>);
 fireEvent.click(screen.getByRole("button",{name:"Load previous generated project ZIP"}));await waitFor(()=>expect(finish).toBeDefined());
 view.rerender(<Theme><RegenerationDialog next={next} open={false} onOpenChange={()=>{}}/></Theme>);
 view.rerender(<Theme><RegenerationDialog next={next} open onOpenChange={()=>{}}/></Theme>);
 await act(async()=>finish({files:[f]}));
 expect(screen.getByRole("button",{name:"Compare and merge"}).hasAttribute("disabled")).toBe(true);
 expect(screen.queryByText("Previous: 1 files; current: 1 files; new: 1 files.")).toBeNull();
 expect(mocks.api.mock.calls.some(([path])=>path.endsWith("/cancel"))).toBe(true);
 expect(screen.getByRole("button",{name:"Load previous generated project ZIP"}).hasAttribute("disabled")).toBe(false);
});
it("does not accept a canceled picker result or expose downloads before conflict choices are applied",async()=>{
 await ready();let finish!:(value:unknown)=>void;mocks.source.mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));
 render(<Theme><RegenerationDialog next={next} open onOpenChange={()=>{}}/></Theme>);
 fireEvent.click(screen.getByRole("button",{name:"Load previous generated project ZIP"}));await screen.findByText("Reading project files…");
 fireEvent.click(screen.getByRole("button",{name:"Cancel"}));
 await act(async()=>finish({kind:"zip",archive_base64:"late"}));
 expect(mocks.api).not.toHaveBeenCalled();expect(screen.getByRole("button",{name:"Download merged project ZIP"}).hasAttribute("disabled")).toBe(true);
});

it("records multiple conflict choices before recomputing and only enables download after the server applies them",async()=>{
 await ready();const files=[f,{...f,path:"run2"}];mocks.pick.mockResolvedValue(JSON.stringify({files}));
 mocks.api.mockImplementation((_path:string,_method:string,input:{resolutions:Record<string,string>})=>Promise.resolve({files:files.map(file=>({...file,status:input.resolutions.run&&input.resolutions.run2?"resolved":"conflict",patch:null})),conflicts:input.resolutions.run&&input.resolutions.run2?0:2,archive_base64:input.resolutions.run&&input.resolutions.run2?"AA==":null}));
 render(<Theme><RegenerationDialog next={{...next,files}} open onOpenChange={()=>{}}/></Theme>);
 fireEvent.click(screen.getByRole("button",{name:"Load previous generation snapshot JSON"}));await waitFor(()=>expect(screen.getByRole("button",{name:"Compare and merge"}).hasAttribute("disabled")).toBe(false));
 fireEvent.click(screen.getByRole("button",{name:"Compare and merge"}));await screen.findByText("Conflicts: 2");
 fireEvent.click(screen.getByRole("button",{name:"Keep current edited version"}));
 fireEvent.change(screen.getByRole("combobox",{name:"Comparison file"}),{target:{value:"run2"}});
 fireEvent.click(screen.getByRole("button",{name:"Use new generated version"}));
 expect(mocks.api).toHaveBeenCalledTimes(1);expect(screen.getByRole("button",{name:"Download merged project ZIP"}).hasAttribute("disabled")).toBe(true);
 fireEvent.click(screen.getByRole("button",{name:"Compare and merge"}));
 await waitFor(()=>expect(screen.getByRole("button",{name:"Download merged project ZIP"}).hasAttribute("disabled")).toBe(false));
 expect(mocks.api).toHaveBeenLastCalledWith("/api/generation/projects/regenerate","POST",expect.objectContaining({resolutions:{run:"working",run2:"generated"}}));
});
