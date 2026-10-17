// @vitest-environment jsdom
import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import SavedDatasets from "./SavedDatasets";
const state=vi.hoisted(()=>({draft:{id:"w",data:{datasets:[] as unknown[]}},dark:false,updateData:vi.fn(),save:vi.fn()}));const api=vi.hoisted(()=>vi.fn());
vi.mock("../workbench/context",()=>({useWorkbench:()=>state}));vi.mock("../../shared/api",()=>({api,pickFile:vi.fn()}));
vi.mock("./RunnerDataset",()=>({default:({value,onChange}:{value:{format:string;source:string};onChange:(value:unknown)=>void})=><textarea aria-label="dataset-source" value={value.source} onChange={event=>onChange({...value,source:event.target.value})}/>}));
beforeEach(async()=>{await setLanguage("en");vi.stubGlobal("ResizeObserver",class{observe(){}unobserve(){}disconnect(){}});Object.defineProperty(HTMLElement.prototype,"scrollIntoView",{configurable:true,value:vi.fn()});state.draft={id:"w",data:{datasets:[]}};state.updateData.mockReset();state.save.mockReset();state.save.mockResolvedValue({});api.mockReset();api.mockResolvedValue({rows:[{id:1}],columns:["id"]});});afterEach(cleanup);
it("validates source then explicitly saves a private dataset using workspace CAS",async()=>{
 render(<Theme><SavedDatasets scope="owner-w-1" disabled={false}/></Theme>);fireEvent.click(screen.getByText("Workspace datasets (0)"));fireEvent.change(screen.getByRole("textbox",{name:"Dataset name"}),{target:{value:"Cases"}});fireEvent.change(screen.getByRole("textbox",{name:"dataset-source"}),{target:{value:'[ {"id":1} ]'}});fireEvent.click(screen.getByRole("button",{name:"Save dataset to workspace"}));await waitFor(()=>expect(state.save).toHaveBeenCalledWith(true));
 expect(api).toHaveBeenCalledWith("/api/testing/dataset/preview","POST",{workspace_id:"w",dataset:{format:"json",source:'[ {"id":1} ]'}});const update=state.updateData.mock.calls[0][0];const result=update({datasets:[]});expect(result.datasets[0]).toEqual(expect.objectContaining({name:"Cases",secret:true,source:{format:"json",source:'[ {"id":1} ]'}}));
});
it("does not commit a source validation finishing after its owner component closes",async()=>{
 let finish!:(value:unknown)=>void;api.mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));const view=render(<Theme><SavedDatasets scope="old-owner" disabled={false}/></Theme>);fireEvent.click(screen.getByText("Workspace datasets (0)"));fireEvent.change(screen.getByRole("textbox",{name:"Dataset name"}),{target:{value:"Private"}});fireEvent.change(screen.getByRole("textbox",{name:"dataset-source"}),{target:{value:'[{"id":1}]'}});fireEvent.click(screen.getByRole("button",{name:"Save dataset to workspace"}));await waitFor(()=>expect(finish).toBeTypeOf("function"));view.unmount();await act(async()=>finish({rows:[{id:1}]}));expect(state.updateData).not.toHaveBeenCalled();
});
it("saves a private query reference without copying SQL or credentials into the dataset",async()=>{
 Object.assign(state.draft.data,{collections:[{id:"data",name:"Sources",requests:[{id:"query-1",name:"Rows",protocol:{kind:"data",read_only:true,sql:"SELECT secret FROM data",url:"https://private.invalid"}}]}]});
 render(<Theme><SavedDatasets scope="owner-w-query" disabled={false}/></Theme>);
 fireEvent.click(screen.getByText("Workspace datasets (0)"));
 fireEvent.change(screen.getByRole("textbox",{name:"Dataset name"}),{target:{value:"Fresh rows"}});
 fireEvent.keyDown(screen.getByRole("combobox",{name:"Dataset source"}),{key:"Enter"});
 fireEvent.click(screen.getByRole("option",{name:"Data query (refresh each run)"}));
 expect(screen.getByRole<HTMLButtonElement>("button",{name:"Save dataset to workspace"}).disabled).toBe(true);
 fireEvent.keyDown(screen.getByRole("combobox",{name:"Select a read-only Data request"}),{key:"Enter"});
 fireEvent.click(screen.getByRole("option",{name:"Sources / Rows"}));
 expect(screen.getByRole<HTMLButtonElement>("button",{name:"Save dataset to workspace"}).disabled).toBe(false);
 fireEvent.click(screen.getByRole("button",{name:"Save dataset to workspace"}));
 await waitFor(()=>expect(state.save).toHaveBeenCalledWith(true));
 expect(api).not.toHaveBeenCalled();
 const update=state.updateData.mock.calls[0][0];
 const dataset=update({datasets:[]}).datasets[0];
 expect(dataset).toEqual(expect.objectContaining({name:"Fresh rows",secret:true,query:{request_id:"query-1"}}));
 expect(dataset.source).toBeUndefined();
 expect(JSON.stringify(dataset)).not.toContain("SELECT secret");
});
