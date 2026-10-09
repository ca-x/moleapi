// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {QueryClient,QueryClientProvider} from "@tanstack/react-query";
import {Theme} from "@radix-ui/themes";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import NotificationsPanel from "./NotificationsPanel";
const api=vi.hoisted(()=>vi.fn());const state=vi.hoisted(()=>({accountId:"owner",authenticated:true,draft:{id:"w"}}));
vi.mock("../workbench/context",()=>({useWorkbench:()=>state}));vi.mock("../../shared/api",()=>({api}));vi.mock("../../shared/ui",()=>({Field:({label,children}:{label:string;children:React.ReactNode})=><label>{label}{children}</label>,Choice:({label,value,options,onChange,disabled}:{label:string;value:string;options:{value:string;label:string}[];onChange:(value:string)=>void;disabled:boolean})=><select aria-label={label} value={value} onChange={event=>onChange(event.target.value)} disabled={disabled}>{options.map(option=><option key={option.value} value={option.value}>{option.label}</option>)}</select>}));
const view=()=>render(<QueryClientProvider client={new QueryClient({defaultOptions:{queries:{retry:false}}})}><Theme><NotificationsPanel/></Theme></QueryClientProvider>);
beforeEach(async()=>{await setLanguage("en");api.mockReset();api.mockResolvedValue([]);});afterEach(cleanup);
it("creates a private signed webhook and exposes all supported channel choices",async()=>{
 view();fireEvent.click(screen.getByText("Notification channels and delivery history"));fireEvent.change(screen.getByRole("textbox",{name:"Notification target name"}),{target:{value:"Failures"}});fireEvent.change(screen.getByLabelText("Private service URL"),{target:{value:"https://example.test/hook?token=private"}});fireEvent.change(screen.getByLabelText("Signing key / Jenkins Bearer"),{target:{value:"secret"}});for(const label of ["Slack","Teams Workflow","WeCom","DingTalk","Feishu","Jenkins","PagerDuty","SMTP Email"])expect(screen.getByRole("option",{name:label})).toBeTruthy();fireEvent.click(screen.getByRole("button",{name:"Save notification target"}));await waitFor(()=>expect(api).toHaveBeenCalledWith("/api/workspaces/w/notifications","POST",{settings:{name:"Failures",kind:"webhook",enabled:false,statuses:["failed"],changes_only:false,language:"en"},credentials:{endpoint:"https://example.test/hook?token=private",signing_secret:"secret",routing_key:""}}));
});
it("updates metadata without sending blank replacements for withheld credentials",async()=>{
 const settings={name:"Existing",kind:"webhook",enabled:true,statuses:["failed"],changes_only:false,language:"en"};api.mockImplementation(async(path:string)=>path.endsWith("/notification-deliveries")?[]:[{id:"target",revision:2,settings,origin:"https://example.test",has_endpoint:true,has_signing_secret:true,smtp:null}]);view();fireEvent.click(screen.getByText("Notification channels and delivery history"));fireEvent.click(await screen.findByRole("button",{name:"Edit notification channel"}));fireEvent.change(screen.getByRole("textbox",{name:"Notification target name"}),{target:{value:"Renamed"}});fireEvent.click(screen.getByRole("button",{name:"Save notification target"}));await waitFor(()=>expect(api).toHaveBeenCalledWith("/api/workspaces/w/notifications/target","PUT",{settings:{...settings,name:"Renamed"},expected_revision:2}));
});
