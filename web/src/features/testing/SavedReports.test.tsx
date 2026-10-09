// @vitest-environment jsdom
import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {QueryClient,QueryClientProvider} from "@tanstack/react-query";
import {Theme} from "@radix-ui/themes";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import SavedReports from "./SavedReports";
const api=vi.hoisted(()=>vi.fn());
const saveFile=vi.hoisted(()=>vi.fn());
const state=vi.hoisted(()=>({accountId:"owner",authenticated:true,draft:{id:"w",data:{collections:[{id:"c",requests:[{id:"r"}]}]}},setGuard:vi.fn(),setRequestId:vi.fn(),setView:vi.fn()}));
vi.mock("../workbench/context",()=>({useWorkbench:()=>state}));vi.mock("../../shared/api",()=>({api}));vi.mock("../generation/saveProjectFile",()=>({saveProjectFile:saveFile}));vi.mock("../../shared/ui",()=>({Choice:({label,value,options,onChange,disabled}:{label:string;value:string;options:{value:string;label:string}[];onChange:(value:string)=>void;disabled:boolean})=><select aria-label={label} value={value} onChange={event=>onChange(event.target.value)} disabled={disabled}>{options.map(option=><option key={option.value} value={option.value}>{option.label}</option>)}</select>}));
const summary={passed:0,failed:1,skipped:0,elapsed_ms:4,iteration_count:1,completed_iterations:1,executed_steps:1,tests_passed:0,tests_failed:1,diagnostics_omitted:0,stopped_reason:null,cancelled:false};
const brief={id:"report",workspace_revision:7,collection_name:"Collection",scenario_name:"Login flow",environment_name:"Dev",started_at:"2026-10-10T00:00:00Z",finished_at:"2026-10-10T00:00:01Z",summary};
const detail={...brief,workspace_id:"w",results:[{position:0,request_id:"r",request_name:"Request",collection_id:"c",method:"GET",iteration:0,step_name:null,step_group:null,parallel_name:null,outcome:"failed",status:200,elapsed_ms:4,size_bytes:5,tests:[{id:"t",name:"Status check",passed:false,actual:"1",expected:"2"}],tests_passed:0,tests_failed:1,diagnostics_omitted:0,error:null}]};
const view=()=>render(<QueryClientProvider client={new QueryClient({defaultOptions:{queries:{retry:false}}})}><Theme><SavedReports disabled={false}/></Theme></QueryClientProvider>);
beforeEach(async()=>{await setLanguage("en");api.mockReset();api.mockImplementation(async(path:string)=>path.includes("/export?")?{filename:"report.html",content:"<html>redacted</html>",mime:"text/html"}:path.endsWith("/reports/report")?detail:{items:[brief],next_cursor:"older-cursor"});saveFile.mockReset();saveFile.mockResolvedValue(undefined);state.setRequestId.mockReset();state.setView.mockReset();});afterEach(cleanup);
it("opens an owned saved report and locates the current failed interface",async()=>{
 view();fireEvent.click(screen.getByText("Saved test reports"));fireEvent.click(await screen.findByRole("button",{name:"View report"}));await screen.findByRole("button",{name:"Open current interface"});expect(screen.getByText(/Source workspace revision/)).toBeTruthy();fireEvent.click(screen.getByRole("button",{name:"Open current interface"}));expect(state.setRequestId).toHaveBeenCalledWith("r");expect(state.setView).toHaveBeenCalledWith("requests");expect(api).toHaveBeenCalledWith("/api/workspaces/w/reports/report");
});
it("downloads a server-projected export using the fenced shared file saver",async()=>{
 view();fireEvent.click(screen.getByText("Saved test reports"));fireEvent.click(await screen.findByRole("button",{name:"View report"}));fireEvent.click(await screen.findByRole("button",{name:"Download redacted report"}));await waitFor(()=>expect(saveFile).toHaveBeenCalled());expect(api).toHaveBeenCalledWith("/api/workspaces/w/reports/report/export?format=html&language=en");expect(saveFile.mock.calls[0][0]).toBe("report.html");expect(saveFile.mock.calls[0][2]()).toBe(true);fireEvent.click(screen.getByRole("button",{name:"Close"}));expect(saveFile.mock.calls[0][2]()).toBe(false);
});
it("discards an export response arriving after the report dialog closes",async()=>{
 let finish!:(value:unknown)=>void;const current=api.getMockImplementation()!;api.mockImplementation((path:string)=>path.includes("/export?")?new Promise(resolve=>{finish=resolve;}):current(path));view();fireEvent.click(screen.getByText("Saved test reports"));fireEvent.click(await screen.findByRole("button",{name:"View report"}));fireEvent.click(await screen.findByRole("button",{name:"Download redacted report"}));await waitFor(()=>expect(finish).toBeTypeOf("function"));fireEvent.click(screen.getByRole("button",{name:"Close"}));await act(async()=>finish({filename:"private.html",content:"old response"}));expect(saveFile).not.toHaveBeenCalled();
});
