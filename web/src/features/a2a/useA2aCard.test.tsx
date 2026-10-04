// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { api } from "../../shared/api";
import { initialData, newRequest } from "../../shared/model";
import { useWorkbench } from "../workbench/context";
import { useA2aCard } from "./useA2aCard";
import { a2aConfig } from "./model";
vi.mock("../../shared/api",()=>({api:vi.fn()}));
vi.mock("../workbench/context",()=>({useWorkbench:vi.fn()}));
beforeEach(()=>{vi.mocked(api).mockReset();});
function setup(){
  const data=initialData();data.specifications=[{id:"card",name:"Agent",kind:"a2a-agent-card",dialect:"a2a-0.3",source:'{"name":"source"}'}];
  const state={authenticated:true,accountId:"owner",draft:{id:"workspace",data},request:{...newRequest(),id:"request",specification_id:"card",protocol:a2aConfig()},localVariables:{values:()=>[]},updateData:vi.fn(),updateRequest:vi.fn()};
  vi.mocked(useWorkbench).mockImplementation(()=>state as unknown as ReturnType<typeof useWorkbench>);return state;
}
it("restores a saved source without making an external card-discovery request",async()=>{
  const state=setup();const result={specification:state.draft.data.specifications![0],card:{name:"saved"},dialect:"0.3",interfaces:[],warnings:[]};
  vi.mocked(api).mockResolvedValue(result);const hook=renderHook(useA2aCard);
  await act(async()=>{await Promise.resolve();});
  expect(hook.result.current.card?.card.name).toBe("saved");
  expect(api).toHaveBeenCalledWith("/api/a2a/cards/import","POST",expect.objectContaining({source:'{"name":"source"}'}));
  expect(api).not.toHaveBeenCalledWith("/api/a2a/cards/discover",expect.anything(),expect.anything());
});
it("rejects a late card result after ownership or dialect changes",async()=>{
  const state=setup();const resolvers:((value:unknown)=>void)[]=[];
  vi.mocked(api).mockImplementation(()=>new Promise(done=>resolvers.push(done)));
  const hook=renderHook(useA2aCard);state.accountId="other";state.request.protocol.dialect="1.0";
  await act(async()=>{hook.rerender();});
  await act(async()=>{resolvers[0]({specification:state.draft.data.specifications![0],card:{name:"private-old"},dialect:"0.3",interfaces:[],warnings:[]});});
  expect(hook.result.current.card).toBeNull();expect(state.updateRequest).not.toHaveBeenCalled();
});
