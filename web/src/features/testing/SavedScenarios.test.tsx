// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import SavedScenarios from "./SavedScenarios";
const state=vi.hoisted(()=>({draft:{id:"w",data:{scenarios:[],collections:[{id:"c",name:"Root",requests:[{id:"a",name:"Login"},{id:"b",name:"Read"}]},{id:"child",parent_id:"c",name:"Child",requests:[{id:"child-request",name:"Nested"}]},{id:"other",name:"Other",requests:[{id:"excluded",name:"Excluded"}]}]}},updateData:vi.fn(),save:vi.fn()}));
vi.mock("../workbench/context",()=>({useWorkbench:()=>state}));
vi.mock("../../shared/ui",()=>({Field:({label,children}:{label:string;children:React.ReactNode})=><label>{label}{children}</label>,Choice:({label,value,options,onChange,disabled}:{label:string;value:string;options:{value:string;label:string}[];onChange:(value:string)=>void;disabled:boolean})=><select aria-label={label} value={value} onChange={event=>onChange(event.target.value)} disabled={disabled}>{options.map(option=><option key={option.value} value={option.value}>{option.label}</option>)}</select>,ToolButton:({label,children,onClick,disabled}:{label:string;children:React.ReactNode;onClick:()=>void;disabled:boolean})=><button aria-label={label} onClick={onClick} disabled={disabled}>{children}</button>}));
beforeEach(async()=>{await setLanguage("en");state.updateData.mockReset();state.save.mockReset();state.save.mockResolvedValue({});});afterEach(cleanup);
it("saves reordered repeated canonical references and groups through workspace CAS",async()=>{
 render(<Theme><SavedScenarios collectionId="c" disabled={false}/></Theme>);fireEvent.click(screen.getByText("Workspace test scenarios (0)"));fireEvent.change(screen.getByRole("textbox",{name:"Scenario name"}),{target:{value:"Auth flow"}});
 for(let i=0;i<3;i++)fireEvent.click(screen.getByRole("button",{name:"Add request step"}));
 const picks=screen.getAllByRole("combobox",{name:"Scenario request"});fireEvent.change(picks[1],{target:{value:"b"}});fireEvent.change(screen.getAllByRole("textbox",{name:"Step group"})[1],{target:{value:"Read group"}});fireEvent.click(screen.getAllByRole("button",{name:"Move step up"})[1]);
 expect(screen.getAllByRole("combobox",{name:"Scenario request"})[0]).toHaveProperty("value","b");expect(screen.queryAllByRole("option",{name:"Other / Excluded"})).toHaveLength(0);expect(screen.getAllByRole("option",{name:"Child / Nested"})).toHaveLength(3);
 fireEvent.click(screen.getByRole("button",{name:"Save scenario to workspace"}));await waitFor(()=>expect(state.save).toHaveBeenCalledWith(true));const next=state.updateData.mock.calls[0][0]({scenarios:[]});expect(next.scenarios[0]).toEqual(expect.objectContaining({collection_id:"c",name:"Auth flow"}));expect(next.scenarios[0].steps.map((step:{request_id:string})=>step.request_id)).toEqual(["b","a","a"]);expect(next.scenarios[0].steps[0].group).toBe("Read group");
});
