// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {QueryClient,QueryClientProvider} from "@tanstack/react-query";
import {Theme} from "@radix-ui/themes";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import SchedulesPanel from "./SchedulesPanel";
const api=vi.hoisted(()=>vi.fn());const state=vi.hoisted(()=>({accountId:"owner",authenticated:true,draft:{id:"w",data:{collections:[{id:"c",name:"Collection"}],scenarios:[{id:"s",name:"Scenario",collection_id:"c"}],environments:[{id:"dev",name:"Dev"}],datasets:[{id:"data",name:"Cases",source:{format:"json",source:"[]"}}]}}}));
vi.mock("../workbench/context",()=>({useWorkbench:()=>state}));vi.mock("../../shared/api",()=>({api,native:false,token:()=>"credential"}));vi.mock("../../shared/ui",()=>({Field:({label,children}:{label:string;children:React.ReactNode})=><label>{label}{children}</label>,Choice:({label,value,options,onChange,disabled}:{label:string;value:string;options:{value:string;label:string}[];onChange:(value:string)=>void;disabled:boolean})=><select aria-label={label} value={value} onChange={event=>onChange(event.target.value)} disabled={disabled}>{options.map(option=><option key={option.value} value={option.value}>{option.label}</option>)}</select>}));
const view=(onReport=vi.fn())=>render(<QueryClientProvider client={new QueryClient({defaultOptions:{queries:{retry:false}}})}><Theme><SchedulesPanel onReport={onReport}/></Theme></QueryClientProvider>);
beforeEach(async()=>{await setLanguage("en");api.mockReset();api.mockImplementation(async(path:string)=>path.endsWith("/preview")?{next:["2026-10-11T01:00:00Z"]}:[]);});afterEach(cleanup);
it("previews cron and persists selected canonical references without variable copies",async()=>{
 view();fireEvent.click(screen.getByText("Schedules and monitoring"));fireEvent.change(screen.getByRole("textbox",{name:"Task name"}),{target:{value:"Daily API"}});fireEvent.change(screen.getByRole("textbox",{name:"IANA timezone"}),{target:{value:"Asia/Shanghai"}});fireEvent.change(screen.getByRole("combobox",{name:"Task scenario"}),{target:{value:"s"}});fireEvent.change(screen.getByRole("combobox",{name:"Task environment"}),{target:{value:"dev"}});fireEvent.change(screen.getByRole("combobox",{name:"Task dataset"}),{target:{value:"data"}});fireEvent.click(screen.getByRole("button",{name:"Preview upcoming times"}));await waitFor(()=>expect(api).toHaveBeenCalledWith("/api/workspaces/w/schedules/preview","POST",{cron:"0 9 * * *",timezone:"Asia/Shanghai"}));await waitFor(()=>expect(screen.getByRole("button",{name:"Save scheduled task"})).toHaveProperty("disabled",false));fireEvent.click(screen.getByRole("button",{name:"Save scheduled task"}));await waitFor(()=>expect(api).toHaveBeenCalledWith("/api/workspaces/w/schedules","POST",{runner_id:null,runner_label:null,name:"Daily API",cron:"0 9 * * *",timezone:"Asia/Shanghai",enabled:false,collection_id:"c",scenario_id:"s",environment_id:"dev",dataset_id:"data",iterations:null}));
});
it("opens the stored report from a completed task",async()=>{
 api.mockImplementation(async(path:string)=>path.endsWith("/notifications")?[]:[{id:"schedule",revision:1,definition:{name:"Daily",cron:"0 9 * * *",timezone:"UTC",enabled:false},running:null,queued:null,last_run:{status:"passed",report_id:"report"}}]);const onReport=vi.fn();view(onReport);fireEvent.click(screen.getByText("Schedules and monitoring"));fireEvent.click(await screen.findByRole("button",{name:"View task report"}));expect(onReport).toHaveBeenCalledWith("report");
});
it("persists an owned self-hosted target and preserves server execution as an explicit alternative",async()=>{
 api.mockImplementation(async(path:string)=>path==="/api/runners"?[{id:"lan",name:"Private LAN",enabled:true}]:[]);view();fireEvent.click(screen.getByText("Schedules and monitoring"));await screen.findByRole("option",{name:"Private LAN"});fireEvent.change(screen.getByRole("combobox",{name:"Scheduled execution target"}),{target:{value:"lan"}});fireEvent.change(screen.getByRole("textbox",{name:"Task name"}),{target:{value:"Remote daily"}});fireEvent.click(screen.getByRole("button",{name:"Save scheduled task"}));await waitFor(()=>expect(api).toHaveBeenCalledWith("/api/workspaces/w/schedules","POST",expect.objectContaining({runner_id:"lan",name:"Remote daily"})));await waitFor(()=>expect(screen.getByRole("button",{name:"Save scheduled task"})).toHaveProperty("disabled",true));fireEvent.change(screen.getByRole("textbox",{name:"Task name"}),{target:{value:"Server daily"}});fireEvent.change(screen.getByRole("combobox",{name:"Scheduled execution target"}),{target:{value:"none"}});fireEvent.click(screen.getByRole("button",{name:"Save scheduled task"}));await waitFor(()=>expect(api).toHaveBeenCalledWith("/api/workspaces/w/schedules","POST",expect.objectContaining({runner_id:null,name:"Server daily"})));
});
it("saves a label group as a remote schedule target and clears it when selecting the server",async()=>{
 api.mockImplementation(async(path:string)=>path==="/api/runners"?[{id:"lan",name:"Private LAN",enabled:true,labels:["internal"]}]:[]);
 view();fireEvent.click(screen.getByText("Schedules and monitoring"));
 await screen.findByRole("option",{name:"Label group #internal"});
 fireEvent.change(screen.getByRole("combobox",{name:"Scheduled execution target"}),{target:{value:"label:internal"}});
 fireEvent.change(screen.getByRole("textbox",{name:"Task name"}),{target:{value:"Internal checks"}});
 fireEvent.click(screen.getByRole("button",{name:"Save scheduled task"}));
 await waitFor(()=>expect(api).toHaveBeenCalledWith("/api/workspaces/w/schedules","POST",expect.objectContaining({runner_id:null,runner_label:"internal",name:"Internal checks"})));
 await waitFor(()=>expect(screen.getByRole("button",{name:"Save scheduled task"})).toHaveProperty("disabled",true));
 fireEvent.change(screen.getByRole("textbox",{name:"Task name"}),{target:{value:"Server checks"}});
 fireEvent.change(screen.getByRole("combobox",{name:"Scheduled execution target"}),{target:{value:"none"}});
 fireEvent.click(screen.getByRole("button",{name:"Save scheduled task"}));
 await waitFor(()=>expect(api).toHaveBeenCalledWith("/api/workspaces/w/schedules","POST",expect.objectContaining({runner_id:null,runner_label:null,name:"Server checks"})));
});
