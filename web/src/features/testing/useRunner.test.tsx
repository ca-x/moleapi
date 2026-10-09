// @vitest-environment jsdom
import {act,renderHook,waitFor} from "@testing-library/react";
import {QueryClient,QueryClientProvider} from "@tanstack/react-query";
import {expect,it,vi} from "vitest";
import {useRunner} from "./useRunner";
const api=vi.hoisted(()=>vi.fn());vi.mock("../../shared/api",()=>({api}));vi.mock("sonner",()=>({toast:{error:vi.fn()}}));
const wrap=({children}:{children:React.ReactNode})=><QueryClientProvider client={new QueryClient()}>{children}</QueryClientProvider>;
it("applies updates to the actual collection for each returned iteration step",async()=>{
 const draft={id:"w",data:{collections:[{id:"root"}],active_environment_id:null}};const workspace={draft,dirty:false,save:vi.fn(),stateRef:{current:{draft}},accountId:"owner",accountRef:{current:"owner"}} as unknown as Parameters<typeof useRunner>[0];const apply=vi.fn();
 api.mockResolvedValue({results:[{request_id:"r",collection_id:"child",iteration:0,response:{variable_updates:[{scope:"collection",key:"value",value:"next"}]}}],passed:1,failed:0,elapsed_ms:1});const {result}=renderHook(()=>useRunner(workspace,{values:()=>[],apply} as unknown as Parameters<typeof useRunner>[1]),{wrapper:wrap});
 await act(async()=>result.current.run({iterations:2,dataset:{format:"csv",source:"id\n1\n2"}}));expect(api).toHaveBeenCalledWith("/api/workspaces/w/run","POST",expect.objectContaining({iterations:2,job_id:expect.any(String)}));expect(apply).toHaveBeenCalledWith(draft,"child",null,[{scope:"collection",key:"value",value:"next"}]);
});
it("does not submit a run cancelled while its save is pending",async()=>{
 let finish!:(value:boolean)=>void;const draft={id:"w",data:{collections:[{id:"root"}],active_environment_id:null}};const save=vi.fn(()=>new Promise<boolean>(resolve=>{finish=resolve;}));const workspace={draft,dirty:true,save,stateRef:{current:{draft}},accountId:"owner",accountRef:{current:"owner"}} as unknown as Parameters<typeof useRunner>[0];api.mockReset();api.mockResolvedValue({cancelled:true});const {result}=renderHook(()=>useRunner(workspace),{wrapper:wrap});
 let pending!:Promise<void>;act(()=>{pending=result.current.run();});await waitFor(()=>expect(save).toHaveBeenCalled());await act(async()=>result.current.stopRunner());await act(async()=>{finish(true);await pending;});expect(api.mock.calls.some(([path])=>path==="/api/workspaces/w/run")).toBe(false);expect(result.current.runResult?.cancelled).toBe(true);
});
