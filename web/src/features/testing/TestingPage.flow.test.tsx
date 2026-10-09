// @vitest-environment jsdom
import {cleanup,render,screen} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import TestingPage from "./TestingPage";
vi.mock("../workbench/context",()=>({useWorkbench:()=>({accountId:"owner",draft:{id:"w",revision:1,data:{collections:[{id:"c",name:"Collection"}]}},runCollection:"c",setRunCollection:vi.fn(),runnerBusy:false,run:vi.fn(),stopRunner:vi.fn(),dark:false,runResult:{results:[{request_id:"conditional",request_name:"Conditional",condition_skipped:true},{request_id:"r",request_name:"Skipped request",iteration:0,response:{status:0,status_text:"Skipped",skipped:true,tests:[]}}],passed:0,failed:0,skipped:1,elapsed_ms:1,iterations:[{iteration:0,passed:0,failed:0,elapsed_ms:1,script_stopped:true},{iteration:1,passed:0,failed:0,elapsed_ms:1,scenario_stopped:true}]}})}));
vi.mock("../notifications/NotificationsPanel",()=>({default:()=>null}));
vi.mock("../monitoring/SchedulesPanel",()=>({default:()=>null}));
vi.mock("./SavedReports",()=>({default:()=>null}));
vi.mock("./SavedScenarios",()=>({default:()=>null}));
vi.mock("./RunnerDataset",()=>({default:()=>null,runnerOptions:()=>({})}));vi.mock("./SavedDatasets",()=>({default:()=>null}));
afterEach(()=>{cleanup();vi.unstubAllGlobals();});
it("labels skipped results and script-ended iterations distinctly from HTTP status0",async()=>{
 await setLanguage("en");vi.stubGlobal("ResizeObserver",class{observe(){}unobserve(){}disconnect(){}});render(<Theme><TestingPage/></Theme>);expect(screen.getByText("Request skipped by script.")).toBeTruthy();expect(screen.getByText(/Script ended this iteration/)).toBeTruthy();expect(screen.queryByText("0 Skipped")).toBeNull();expect(screen.getByText("Request skipped because the scenario condition is false.")).toBeTruthy();expect(screen.getByText(/Scenario branch ended this iteration/)).toBeTruthy();
});
