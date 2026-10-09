// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import TestingPage from "./TestingPage";
const run=vi.hoisted(()=>vi.fn());
vi.mock("../workbench/context",()=>({useWorkbench:()=>({accountId:"owner",draft:{id:"w",revision:1,data:{collections:[{id:"c",name:"Collection"}],scenarios:[{id:"s",name:"Login flow",collection_id:"c",steps:[]},{id:"excluded",name:"Other root",collection_id:"other",steps:[]}]}},runCollection:"c",setRunCollection:vi.fn(),runResult:null,runnerBusy:false,run,stopRunner:vi.fn(),dark:false})}));
vi.mock("../../shared/ui",()=>({Choice:({label,value,options,onChange}:{label:string;value:string;options:{value:string;label:string}[];onChange:(value:string)=>void})=><select aria-label={label} value={value} onChange={event=>onChange(event.target.value)}>{options.map(option=><option key={option.value} value={option.value}>{option.label}</option>)}</select>}));
vi.mock("./RunnerDataset",()=>({default:()=>null,runnerOptions:()=>({iterations:2})}));vi.mock("./SavedDatasets",()=>({default:()=>null}));vi.mock("../monitoring/SchedulesPanel",()=>({default:()=>null}));
vi.mock("./SavedReports",()=>({default:()=>null}));
vi.mock("./SavedScenarios",()=>({default:()=>null}));
afterEach(cleanup);
it("runs only a selected scenario for the current root with dataset options",async()=>{
 await setLanguage("en");render(<Theme><TestingPage/></Theme>);expect(screen.queryByRole("option",{name:"Other root"})).toBeNull();fireEvent.change(screen.getByRole("combobox",{name:"Run scenario"}),{target:{value:"s"}});fireEvent.click(screen.getByRole("button",{name:"Run collection"}));expect(run).toHaveBeenCalledWith({iterations:2,scenario_id:"s"});
});
