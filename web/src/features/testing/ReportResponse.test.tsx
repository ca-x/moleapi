// @vitest-environment jsdom
import {act,cleanup,render,screen} from "@testing-library/react";
import {QueryClient,QueryClientProvider} from "@tanstack/react-query";
import {Theme} from "@radix-ui/themes";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import ReportResponse from "./ReportResponse";
const api=vi.hoisted(()=>vi.fn());vi.mock("../../shared/api",()=>({api}));vi.mock("../requests/ResponsePane",()=>({ResponsePane:({response,error}:{response:{body:string}|null;error:string})=>error?<div role="alert">{error}</div>:<div>{response?.body}</div>}));
beforeEach(async()=>{await setLanguage("en");api.mockReset();});afterEach(cleanup);
it("fetches the referenced stored response and distinguishes cleared history",async()=>{
 api.mockResolvedValue({response:{body:"redacted historical response"}});const client=new QueryClient({defaultOptions:{queries:{retry:false}}});const content=(position:number)=><QueryClientProvider client={client}><Theme><ReportResponse accountId="owner" workspaceId="w" reportId="report" position={position} dark={false} onClose={()=>{}}/></Theme></QueryClientProvider>;const view=render(content(0));await screen.findByText("redacted historical response");expect(api).toHaveBeenCalledWith("/api/workspaces/w/reports/report/steps/0/response");api.mockRejectedValue(Object.assign(new Error("missing"),{status:404}));view.rerender(content(1));expect(await screen.findByRole("alert")).toHaveProperty("textContent","Historical response was deleted or is unavailable; the report remains.");
});
it("does not display a previous report response finishing after the selection changes",async()=>{
 let finish!:(value:unknown)=>void;api.mockImplementation((path:string)=>path.includes("/old/")?new Promise(resolve=>{finish=resolve;}):Promise.resolve({response:{body:"current response"}}));const client=new QueryClient({defaultOptions:{queries:{retry:false}}});const content=(reportId:string)=><QueryClientProvider client={client}><Theme><ReportResponse accountId="owner" workspaceId="w" reportId={reportId} position={0} dark={false} onClose={()=>{}}/></Theme></QueryClientProvider>;const view=render(content("old"));view.rerender(content("current"));await screen.findByText("current response");await act(async()=>finish({response:{body:"stale response"}}));expect(screen.queryByText("stale response")).toBeNull();
});
