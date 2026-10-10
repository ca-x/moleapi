// @vitest-environment jsdom
import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {QueryClient,QueryClientProvider} from "@tanstack/react-query";
import {Theme} from "@radix-ui/themes";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {api,token} from "../../shared/api";
import AccessTokensPanel from "./AccessTokensPanel";
vi.mock("../../shared/api",()=>({api:vi.fn(),token:vi.fn()}));
vi.mock("../../shared/ui",()=>({Field:({label,children}:{label:string;children:React.ReactNode})=><label>{label}{children}</label>,Choice:({label,value,options,onChange}:{label:string;value:string;options:{value:string;label:string}[];onChange:(value:string)=>void})=><label>{label}<select value={value} onChange={event=>onChange(event.target.value)}>{options.map(option=><option key={option.value} value={option.value}>{option.label}</option>)}</select></label>}));
const metadata={id:"key",name:"CI",created_at:1,expires_at:2000000000,expired:false};
function view(account="owner"){return <QueryClientProvider client={client}><Theme><AccessTokensPanel accountId={account}/></Theme></QueryClientProvider>;}
let client:QueryClient;
beforeEach(()=>{client=new QueryClient({defaultOptions:{queries:{retry:false}}});vi.mocked(api).mockReset();vi.mocked(token).mockReturnValue("session-owner");Object.defineProperty(navigator,"clipboard",{configurable:true,value:{writeText:vi.fn().mockResolvedValue(undefined)}});});
afterEach(()=>{cleanup();client.clear();});
it("shows one-time plaintext without caching it and confirms revocation",async()=>{
 vi.mocked(api).mockImplementation((_path,method)=>Promise.resolve(method==="POST"?{...metadata,token:"one-time-secret"}:method==="DELETE"?{revoked:true}:[metadata]));
 render(view());await screen.findByText("CI");fireEvent.change(screen.getByLabelText("令牌名称"),{target:{value:"Build"}});fireEvent.click(screen.getByRole("button",{name:"创建访问令牌"}));
 const secret=await screen.findByLabelText("新访问令牌");expect((secret as HTMLTextAreaElement).value).toBe("one-time-secret");
 expect(JSON.stringify(client.getQueryCache().getAll().map(query=>query.state.data))).not.toContain("one-time-secret");
 fireEvent.click(screen.getByRole("button",{name:"复制令牌"}));await waitFor(()=>expect(navigator.clipboard.writeText).toHaveBeenCalledWith("one-time-secret"));
 fireEvent.click(screen.getByRole("button",{name:"隐藏令牌"}));expect(screen.queryByLabelText("新访问令牌")).toBeNull();
 fireEvent.click(screen.getByRole("button",{name:"撤销令牌"}));expect(api).not.toHaveBeenCalledWith("/api/auth/tokens/key","DELETE");
 fireEvent.click(screen.getByRole("button",{name:"确认撤销"}));await waitFor(()=>expect(api).toHaveBeenCalledWith("/api/auth/tokens/key","DELETE"));
});
it("discards a pending secret after changing credentials even for the same account",async()=>{
 let resolve!:(value:unknown)=>void;vi.mocked(api).mockImplementation((_path,method)=>method==="POST"?new Promise(done=>{resolve=done;}):Promise.resolve([]));
 const rendered=render(view());fireEvent.change(screen.getByLabelText("令牌名称"),{target:{value:"Late"}});fireEvent.click(screen.getByRole("button",{name:"创建访问令牌"}));
 await waitFor(()=>expect(api).toHaveBeenCalledWith("/api/auth/tokens","POST",expect.anything()));
 vi.mocked(token).mockReturnValue("another-session");rendered.rerender(view());
 await act(async()=>resolve({...metadata,token:"stale-private-token"}));expect(screen.queryByLabelText("新访问令牌")).toBeNull();expect(document.body.textContent).not.toContain("stale-private-token");
});
