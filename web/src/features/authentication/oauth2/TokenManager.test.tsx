// @vitest-environment jsdom
import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {api} from "../../../shared/api";
import {setLanguage} from "../../../shared/i18n";
import {initialData} from "../../../shared/model";
import {useWorkbench} from "../../workbench/context";
import TokenManager from "./TokenManager";
import {oauth2Config,type OAuth2Auth} from "./types";
vi.mock("../../../shared/api",()=>({api:vi.fn(),native:false}));
vi.mock("../../workbench/context",()=>({useWorkbench:vi.fn()}));
let state:ReturnType<typeof useWorkbench>;
const token={id:"token-one",label:"Development",client_id:"client",issuer:"https://issuer.test",token_type:"Bearer",scopes:["read"],created_at:1,expires_at:2000000000,has_refresh_token:true,revoked:false,refreshing:false};
beforeEach(()=>{
 vi.stubGlobal("ResizeObserver",class {observe(){} unobserve(){} disconnect(){}});
 Object.defineProperty(HTMLElement.prototype,"scrollIntoView",{configurable:true,value:vi.fn()});
 setLanguage("en");const data=initialData();const request=data.collections[0].requests[0];
 data.collections[0].id="selected-request-collection";
 data.collections.push({id:"edited-parent",name:"Parent",description:"",requests:[]});
 state={accountId:"owner",draft:{id:"workspace",data},request,dirty:false,localVariables:{values:vi.fn(()=>[])},save:vi.fn(async()=>true)} as unknown as ReturnType<typeof useWorkbench>;
 vi.mocked(useWorkbench).mockImplementation(()=>state);
 vi.mocked(api).mockImplementation(async()=>[token] as never);
});
afterEach(()=>{cleanup();vi.resetAllMocks();vi.unstubAllGlobals();});
function view(config:OAuth2Auth={...oauth2Config(),grant:"client_credentials" as const,token_id:token.id},collectionId?:string|null){const select=vi.fn();const result=render(<Theme><TokenManager config={config} select={select} collectionId={collectionId}/></Theme>);fireEvent.click(screen.getByRole("button",{name:"Manage OAuth2 tokens"}));return {select,...result};}
it("uses the edited collection rather than the selected request for acquisition",async()=>{
 vi.mocked(api).mockImplementation(async(path)=>path==="/api/oauth2/tokens/acquire"?token as never:[token] as never);
 const {select}=view(undefined,"edited-parent");await screen.findByText(/Token expires/);
 fireEvent.click(screen.getByRole("button",{name:"Acquire token"}));await waitFor(()=>expect(select).toHaveBeenCalledWith(token.id));
 const call=vi.mocked(api).mock.calls.find(([path])=>path==="/api/oauth2/tokens/acquire")!;
 expect(call[2]).toMatchObject({collection_id:"edited-parent",workspace_id:"workspace"});
 expect(state.localVariables.values).toHaveBeenCalledWith(state.draft,"edited-parent",state.draft!.data.active_environment_id);
});
it("does not inherit selected request collection scopes for workspace authentication",async()=>{
 view(undefined,null);await screen.findByText(/Token expires/);fireEvent.click(screen.getByRole("button",{name:"Acquire token"}));
 await waitFor(()=>expect(api).toHaveBeenCalledWith("/api/oauth2/tokens/acquire","POST",expect.objectContaining({collection_id:null})));
});
it("ignores a secret response after the account changes and clears visible token metadata",async()=>{
 let reveal!:(value:unknown)=>void;
 vi.mocked(api).mockImplementation(async(path)=>path.endsWith("/secret")?new Promise(resolve=>{reveal=resolve;}) as never:[token] as never);
 const config={...oauth2Config(),grant:"client_credentials" as const,token_id:token.id};const {rerender}=view(config);await screen.findByText(/Token expires/);
 fireEvent.click(screen.getByRole("button",{name:"Reveal token"}));await waitFor(()=>expect(reveal).toBeDefined());
 state={...state,accountId:"other"};rerender(<Theme><TokenManager config={config} select={vi.fn()}/></Theme>);
 await act(async()=>reveal({access_token:"late-private-secret"}));
 expect(screen.queryByDisplayValue("late-private-secret")).toBeNull();
});
it("exposes inspection only after explicit action and uses the same scoped configuration",async()=>{
 vi.mocked(api).mockImplementation(async(path)=>path.endsWith("/introspect")?{active:false,expires_at:null,scopes:[]} as never:[token] as never);
 view({...oauth2Config(),grant:"client_credentials",token_id:token.id,introspection_url:"https://issuer.test/introspect"});await screen.findByText(/Token expires/);
 expect(vi.mocked(api).mock.calls.some(([path])=>path.endsWith("/introspect"))).toBe(false);
 fireEvent.click(screen.getByRole("button",{name:"Inspect token"}));await screen.findByText("The provider reports this token is inactive.");
 expect(api).toHaveBeenCalledWith("/api/workspaces/workspace/oauth2/tokens/token-one/introspect","POST",expect.objectContaining({workspace_id:"workspace"}));
});
it("cancels a pending browser flow when its dialog is closed",async()=>{
 vi.mocked(api).mockImplementation(async(path)=>path==="/api/oauth2/flows"?{id:"flow-one",stage:"pending",authorization_url:"https://issuer.test/authorize",expires_at:2000000000} as never:[token] as never);
 view({...oauth2Config(),token_id:null});fireEvent.click(screen.getByRole("button",{name:"Acquire token"}));await screen.findByText(/Awaiting authorization/);
 fireEvent.click(screen.getByRole("button",{name:"Done"}));await waitFor(()=>expect(api).toHaveBeenCalledWith("/api/oauth2/flows/flow-one/cancel","POST"));
});
it("fences a deferred reveal across close and reopen of the same configuration",async()=>{
 let reveal!:(value:unknown)=>void;vi.mocked(api).mockImplementation(async(path)=>path.endsWith("/secret")?new Promise(resolve=>{reveal=resolve;}) as never:[token] as never);
 view();await screen.findByText(/Token expires/);fireEvent.click(screen.getByRole("button",{name:"Reveal token"}));await waitFor(()=>expect(reveal).toBeDefined());
 fireEvent.click(screen.getByRole("button",{name:"Done"}));fireEvent.click(screen.getByRole("button",{name:"Manage OAuth2 tokens"}));
 await act(async()=>reveal({access_token:"closed-dialog-secret"}));expect(screen.queryByDisplayValue("closed-dialog-secret")).toBeNull();
});
it("cancels a newly returned flow after its dialog was closed and reopened",async()=>{
 let begin!:(value:unknown)=>void;vi.mocked(api).mockImplementation(async(path)=>path==="/api/oauth2/flows"?new Promise(resolve=>{begin=resolve;}) as never:[token] as never);
 const {select}=view({...oauth2Config(),token_id:null});fireEvent.click(screen.getByRole("button",{name:"Acquire token"}));await waitFor(()=>expect(begin).toBeDefined());
 fireEvent.click(screen.getByRole("button",{name:"Done"}));fireEvent.click(screen.getByRole("button",{name:"Manage OAuth2 tokens"}));
 await act(async()=>begin({id:"late-flow",stage:"pending",authorization_url:"https://issuer.test/authorize"}));
 expect(api).toHaveBeenCalledWith("/api/oauth2/flows/late-flow/cancel","POST");expect(select).not.toHaveBeenCalled();
});
it("prevents another acquisition while a browser grant is pending",async()=>{
 vi.mocked(api).mockImplementation(async(path)=>path==="/api/oauth2/flows"?{id:"active-flow",stage:"pending",authorization_url:"https://issuer.test/authorize"} as never:[token] as never);
 view({...oauth2Config(),token_id:null});fireEvent.click(screen.getByRole("button",{name:"Acquire token"}));await screen.findByText(/Awaiting authorization/);
 expect((screen.getByRole("button",{name:"Acquire token"}) as HTMLButtonElement).disabled).toBe(true);
});
it("keeps TLS verification enabled for parent token operations",async()=>{
 state.request={...state.request!,verify_tls:false};view(undefined,"edited-parent");await screen.findByText(/Token expires/);fireEvent.click(screen.getByRole("button",{name:"Acquire token"}));
 await waitFor(()=>expect(api).toHaveBeenCalledWith("/api/oauth2/tokens/acquire","POST",expect.objectContaining({verify_tls:true})));
});
it("cancels authorization without waiting for in-flight completion",async()=>{
 const flow={id:"interrupt-flow",stage:"pending",authorization_url:"https://issuer.test/authorize"};let complete!:(value:unknown)=>void;
 vi.mocked(api).mockImplementation(async(path)=>path==="/api/oauth2/flows"?flow as never:path.endsWith("/complete")?new Promise(resolve=>{complete=resolve;}) as never:path.endsWith("/cancel")?{...flow,stage:"cancelled"} as never:[token] as never);
 view({...oauth2Config(),token_id:null});fireEvent.click(screen.getByRole("button",{name:"Acquire token"}));await screen.findByText(/Awaiting authorization/);
 fireEvent.change(screen.getByLabelText("Paste the redirect URL after authorization"),{target:{value:"https://redirect.test/?state=state&code=code"}});
 fireEvent.click(screen.getByRole("button",{name:"Complete authorization"}));await waitFor(()=>expect(complete).toBeDefined());
 fireEvent.click(screen.getByRole("button",{name:"Cancel authorization"}));await waitFor(()=>expect(api).toHaveBeenCalledWith("/api/oauth2/flows/interrupt-flow/cancel","POST"));
 await act(async()=>complete({...flow,stage:"cancelled"}));
});
it("selects the token when polling completes an automatic browser callback",async()=>{
 let loads=0;vi.mocked(api).mockImplementation(async(path)=>path==="/api/oauth2/flows"?{id:"automatic-flow",stage:"pending",authorization_url:"https://issuer.test/authorize"} as never:path==="/api/oauth2/flows/automatic-flow"?{id:"automatic-flow",stage:"completed",token_id:token.id} as never:++loads>1?new Promise(resolve=>setTimeout(()=>resolve([token]),50)) as never:[token] as never);
 const {select}=view({...oauth2Config(),token_id:null});fireEvent.click(screen.getByRole("button",{name:"Acquire token"}));
 await waitFor(()=>expect(select).toHaveBeenCalledWith(token.id),{timeout:3000});
});
