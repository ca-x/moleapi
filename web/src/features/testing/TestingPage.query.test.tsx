// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import TestingPage from "./TestingPage";
const run=vi.hoisted(()=>vi.fn());
vi.mock("../workbench/context",()=>({useWorkbench:()=>({accountId:"owner",draft:{id:"w",revision:1,data:{collections:[{id:"c",name:"Collection",requests:[]}],datasets:[{id:"fresh",name:"Fresh rows",secret:true,query:{request_id:"data-query"}}]}},runCollection:"c",setRunCollection:vi.fn(),runResult:null,runnerBusy:false,run,stopRunner:vi.fn(),dark:false})}));
vi.mock("../../shared/ui",()=>({Choice:({label,value,options,onChange}:{label:string;value:string;options:{value:string;label:string}[];onChange:(value:string)=>void})=><select aria-label={label} value={value} onChange={event=>onChange(event.target.value)}>{options.map(option=><option key={option.value} value={option.value}>{option.label}</option>)}</select>}));
vi.mock("./RunnerDataset",()=>({default:({iterationsOnly}:{iterationsOnly:boolean})=><output>{iterationsOnly?"query iterations only":"source editor"}</output>,runnerOptions:()=>({iterations:2})}));
vi.mock("./SavedDatasets",()=>({default:()=>null}));
vi.mock("./SavedScenarios",()=>({default:()=>null}));
vi.mock("../notifications/NotificationPicker",()=>({default:()=>null}));
vi.mock("../notifications/NotificationsPanel",()=>({default:()=>null}));
vi.mock("../monitoring/SchedulesPanel",()=>({default:()=>null}));
vi.mock("../runners/RunnersPanel",()=>({default:()=>null}));
vi.mock("./SavedReports",()=>({default:()=>null}));
afterEach(()=>{cleanup();run.mockReset();});
it("runs a query-backed dataset by saved ID without requiring a literal source",async()=>{
 await setLanguage("en");render(<Theme><TestingPage/></Theme>);
 fireEvent.change(screen.getByRole("combobox",{name:"Run dataset"}),{target:{value:"fresh"}});
 expect(screen.getByText("query iterations only")).toBeTruthy();
 expect(screen.getByText("Refresh the Data query when the run starts. A failed query stops the collection before any request.")).toBeTruthy();
 fireEvent.click(screen.getByRole("button",{name:"Run collection"}));
 expect(run).toHaveBeenCalledWith({iterations:2,dataset_id:"fresh"});
});
