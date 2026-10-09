// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen,waitFor,act} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import ProjectTemplatePanel,{parseTemplateBundle} from "./ProjectTemplatePanel";
const pick=vi.hoisted(()=>vi.fn());
const call=vi.hoisted(()=>vi.fn());
vi.mock("../../shared/api",()=>({pickFile:pick,api:call,native:false}));
vi.mock("../../shared/ui",async()=>{const real=await vi.importActual<object>("../../shared/ui");return {...real,Editor:({value,onChange}:{value:string;onChange:(value:string)=>void})=><textarea aria-label="template-editor" value={value} onChange={event=>onChange(event.target.value)}/>};});
const bundle={format:"moleapi-codegen-templates-v1" as const,files:[{path:"models.mustache",content:"{{classname}}"}]};
beforeEach(async()=>{await setLanguage("en");vi.stubGlobal("ResizeObserver",class{observe(){}unobserve(){}disconnect(){}});pick.mockReset();call.mockReset();});
afterEach(()=>{cleanup();vi.unstubAllGlobals();});
it("previews binary assets as metadata without editing encoded bytes as a template",()=>{
 const value=parseTemplateBundle({format:"moleapi-codegen-templates-v1",files:[{path:"asset.bin",encoding:"base64",content:"AP+A"}],outputs:{"asset.bin":{}}});
 render(<Theme><ProjectTemplatePanel workspaceId="w" scope="owner" value={value} disabled={false} dark={false} onChange={vi.fn()} onBusyChange={vi.fn()}/></Theme>);
 fireEvent.click(screen.getByText("Custom Mustache templates (1)"));expect(screen.getByText("Binary static assets retain their original bytes and are copied using their output mappings.")).toBeTruthy();expect(screen.queryByRole("textbox",{name:"template-editor"})).toBeNull();
});
it("restores exact template source from a saved generated snapshot and edits through the existing editor",async()=>{
 const change=vi.fn();pick.mockResolvedValue(JSON.stringify({files:[{path:"moleapi-templates.json",encoding:"utf8",content:JSON.stringify(bundle)}]}));
 const props={workspaceId:"w",scope:"owner",value:null,disabled:false,dark:false,onChange:change,onBusyChange:vi.fn()};
 const view=render(<Theme><ProjectTemplatePanel {...props}/></Theme>);
 fireEvent.click(screen.getByText("Custom Mustache templates"));fireEvent.click(screen.getByRole("button",{name:"Load template JSON / generation snapshot"}));
 await waitFor(()=>expect(change).toHaveBeenCalledWith(bundle));
 view.rerender(<Theme><ProjectTemplatePanel {...props} value={bundle}/></Theme>);
 fireEvent.change(screen.getByRole("textbox",{name:"template-editor"}),{target:{value:"custom {{classname}}"}});
 expect(change).toHaveBeenLastCalledWith({...bundle,files:[{path:"models.mustache",content:"custom {{classname}}"}]});
});
it("discards template selections that finish after the owner scope changes",async()=>{
 let finish!:(value:string)=>void;pick.mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));const change=vi.fn();
 const props={workspaceId:"w",scope:"old-owner",value:null,disabled:false,dark:false,onChange:change,onBusyChange:vi.fn()};
 const view=render(<Theme><ProjectTemplatePanel {...props}/></Theme>);
 fireEvent.click(screen.getByText("Custom Mustache templates"));fireEvent.click(screen.getByRole("button",{name:"Load template JSON / generation snapshot"}));
 await waitFor(()=>expect(finish).toBeTypeOf("function"));
 view.rerender(<Theme><ProjectTemplatePanel {...props} scope="new-owner"/></Theme>);
 await act(async()=>finish(JSON.stringify(bundle)));expect(change).not.toHaveBeenCalled();
});
it("bounds UTF-8 bytes and rejects duplicate/traversal template paths",()=>{
 for(const value of [{...bundle,files:[bundle.files[0],bundle.files[0]]},{...bundle,files:[{path:"../models.mustache",content:""}]},{...bundle,files:[{path:"models.mustache",content:"鼹".repeat(24000)}]}])expect(()=>parseTemplateBundle(value)).toThrow();
});
