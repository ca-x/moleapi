// @vitest-environment jsdom
import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {api} from "../../../shared/api";
import {setLanguage} from "../../../shared/i18n";
import {WorkbenchContext} from "../../workbench/context";
import TokenManager from "./TokenManager";
import {oauth1Config} from "./OAuth1AuthEditor";
vi.mock("../../../shared/api",()=>({api:vi.fn(),native:false}));
const state={accountId:"owner",draft:{id:"workspace",data:{active_environment_id:"dev",global_variables:[] as unknown[],environments:[] as unknown[],collections:[{id:"collection",requests:[{id:"r"}]}]}},request:{id:"r",verify_tls:true},dirty:false,save:vi.fn().mockResolvedValue(true),localVariables:{values:()=>[]}};
const config={...oauth1Config(),consumer_key:"consumer",consumer_secret:"{{secret}}",grant:{request_token_url:"https://example.com/request",authorization_url:"https://example.com/auth",access_token_url:"https://example.com/token",callback_url:"oob",request_params:[],access_params:[]}};
const select=vi.fn();
function host(value=config){return <Theme><WorkbenchContext.Provider value={state as unknown as NonNullable<React.ContextType<typeof WorkbenchContext>>}><TokenManager config={value} select={select} callback={vi.fn()}/></WorkbenchContext.Provider></Theme>;}
beforeEach(async()=>{await setLanguage("en");vi.stubGlobal("ResizeObserver",class{observe(){}unobserve(){}disconnect(){}});vi.mocked(api).mockImplementation(async(path)=>{if(path.includes("/tokens"))return [] as never;return {id:"flow",stage:"pending",authorization_url:"https://example.com/auth?oauth_token=t",expires_at:1,token_id:null,error:null} as never;});});
afterEach(()=>{cleanup();vi.clearAllMocks();vi.unstubAllGlobals();});
async function open(){render(host());fireEvent.click(screen.getByRole("button",{name:"Manage OAuth1 tokens"}));await screen.findByRole("button",{name:"Acquire token"});}
it("cancels a late begin response after close/reopen without selecting old credentials",async()=>{
 let finish!:(flow:unknown)=>void;vi.mocked(api).mockImplementation(async(path)=>{if(path==="/api/oauth1/flows")return new Promise(resolve=>{finish=resolve;}) as never;if(path.includes("/tokens"))return [] as never;return {id:"flow",stage:"cancelled"} as never;});
 await open();fireEvent.click(screen.getByRole("button",{name:"Acquire token"}));await waitFor(()=>expect(finish).toBeTypeOf("function"));fireEvent.click(screen.getByRole("button",{name:"Done"}));fireEvent.click(screen.getByRole("button",{name:"Manage OAuth1 tokens"}));
 await act(async()=>{finish({id:"late",stage:"pending",authorization_url:"https://example.com/auth",expires_at:1,token_id:null,error:null});});await waitFor(()=>expect(api).toHaveBeenCalledWith("/api/oauth1/flows/late/cancel","POST"));expect(select).not.toHaveBeenCalled();
});
it("allows cancellation while access-token completion is waiting and ignores its late success",async()=>{
 let finish!:(flow:unknown)=>void;
 vi.mocked(api).mockImplementation(async(path)=>{if(path.includes("/complete"))return new Promise(resolve=>{finish=resolve;}) as never;if(path.includes("/tokens"))return [] as never;return {id:"flow",stage:path.endsWith("/cancel")?"cancelled":"pending",authorization_url:"https://example.com/auth",expires_at:1,token_id:null,error:null} as never;});
 await open();fireEvent.click(screen.getByRole("button",{name:"Acquire token"}));await screen.findByRole("button",{name:"Complete authorization"});fireEvent.change(screen.getByLabelText("OAuth1 verifier / PIN or callback URL"),{target:{value:"pin"}});fireEvent.click(screen.getByRole("button",{name:"Complete authorization"}));await waitFor(()=>expect(finish).toBeTypeOf("function"));
 fireEvent.click(screen.getByRole("button",{name:"Cancel authorization"}));await waitFor(()=>expect(api).toHaveBeenCalledWith("/api/oauth1/flows/flow/cancel","POST"));await screen.findByText(/Authorization cancelled/);
 await act(async()=>{finish({id:"flow",stage:"completed",authorization_url:null,token_id:"late-token"});});expect(select).not.toHaveBeenCalled();
});
it("does not reveal a credential response that belongs to a closed manager",async()=>{
 let finish!:(credential:unknown)=>void;
 vi.mocked(api).mockImplementation(async(path)=>path.endsWith("/secret")?new Promise(resolve=>{finish=resolve;}) as never:[] as never);
 render(host({...config,token_id:"selected"}));fireEvent.click(screen.getByRole("button",{name:"Manage OAuth1 tokens"}));await screen.findByRole("button",{name:"Reveal token"});fireEvent.click(screen.getByRole("button",{name:"Reveal token"}));await waitFor(()=>expect(finish).toBeTypeOf("function"));fireEvent.click(screen.getByRole("button",{name:"Done"}));fireEvent.click(screen.getByRole("button",{name:"Manage OAuth1 tokens"}));await act(async()=>{finish({token:"old-private-token",secret:"old-private-secret"});});expect(screen.queryByDisplayValue("old-private-token")).toBeNull();
});
it("cancels a flow and discards its completion when same-environment variables change",async()=>{
 let finish!:(flow:unknown)=>void;
 vi.mocked(api).mockImplementation(async(path)=>{if(path.includes("/complete"))return new Promise(resolve=>{finish=resolve;}) as never;if(path.includes("/tokens"))return [] as never;return {id:"flow",stage:path.endsWith("/cancel")?"cancelled":"pending",authorization_url:"https://example.com/auth",expires_at:1,token_id:null,error:null} as never;});
 const view=render(host());fireEvent.click(screen.getByRole("button",{name:"Manage OAuth1 tokens"}));await screen.findByRole("button",{name:"Acquire token"});fireEvent.click(screen.getByRole("button",{name:"Acquire token"}));await screen.findByRole("button",{name:"Complete authorization"});fireEvent.change(screen.getByLabelText("OAuth1 verifier / PIN or callback URL"),{target:{value:"pin"}});fireEvent.click(screen.getByRole("button",{name:"Complete authorization"}));await waitFor(()=>expect(finish).toBeTypeOf("function"));
 state.draft.data.global_variables=[{id:"v",key:"client",value:"new-client",enabled:true}];view.rerender(host());await waitFor(()=>expect(api).toHaveBeenCalledWith("/api/oauth1/flows/flow/cancel","POST"));await act(async()=>{finish({id:"flow",stage:"completed",authorization_url:null,token_id:"old-environment-token"});});expect(select).not.toHaveBeenCalled();state.draft.data.global_variables=[];
});
