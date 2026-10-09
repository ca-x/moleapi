// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen,waitFor,act} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import RunnerDataset,{runnerOptions} from "./RunnerDataset";
const api=vi.hoisted(()=>vi.fn());const pick=vi.hoisted(()=>vi.fn());
vi.mock("../../shared/api",()=>({api,pickFile:pick}));
vi.mock("../../shared/ui",async()=>{const actual=await vi.importActual<object>("../../shared/ui");return {...actual,Editor:({value,onChange,label}:{value:string;onChange:(value:string)=>void;label:string})=><textarea aria-label={label} value={value} onChange={event=>onChange(event.target.value)}/>};});
beforeEach(async()=>{await setLanguage("en");vi.stubGlobal("ResizeObserver",class{observe(){}unobserve(){}disconnect(){}});api.mockReset();pick.mockReset();});
afterEach(()=>{cleanup();vi.unstubAllGlobals();});
it("previews typed rows through the owned API and leaves iteration count automatic",async()=>{
 api.mockResolvedValue({columns:["id"],rows:[{id:1},{id:2}]});const value={format:"json" as const,source:'[{"id":1},{"id":2}]',iterations:""};
 render(<Theme><RunnerDataset workspaceId="w" scope="owner" value={value} disabled={false} dark={false} onChange={vi.fn()}/></Theme>);fireEvent.click(screen.getByRole("button",{name:"Preview data rows"}));await screen.findByText("Dataset contains 2 rows and 1 columns.");
 expect(api).toHaveBeenCalledWith("/api/testing/dataset/preview","POST",{workspace_id:"w",dataset:{format:"json",source:value.source}});expect(runnerOptions(value)).toEqual({dataset:{format:"json",source:value.source}});
});
it("discards a selected private file after the owner changes",async()=>{
 let finish!:(value:string)=>void;pick.mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));const change=vi.fn();const props={workspaceId:"w",scope:"old-owner",value:{format:"csv" as const,source:"",iterations:"2"},disabled:false,dark:false,onChange:change};
 const view=render(<Theme><RunnerDataset {...props}/></Theme>);fireEvent.click(screen.getByRole("button",{name:"Import test data"}));await waitFor(()=>expect(finish).toBeTypeOf("function"));view.rerender(<Theme><RunnerDataset {...props} scope="new-owner"/></Theme>);await act(async()=>finish("secret\nprivate-value"));expect(change).not.toHaveBeenCalled();expect(pick).toHaveBeenCalledWith(expect.objectContaining({maximum:1024*1024}));
});
it("rejects invalid counts and oversized UTF-8 sources before execution",()=>{
 for(const iterations of ["0","101","1.5","invalid"])expect(()=>runnerOptions({format:"json",source:"[]",iterations})).toThrow();expect(()=>runnerOptions({format:"csv",source:"鼹".repeat(400000),iterations:""})).toThrow();
});
