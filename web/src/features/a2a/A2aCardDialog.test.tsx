// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { api } from "../../shared/api";
import { useWorkbench } from "../workbench/context";
import A2aCardDialog from "./A2aCardDialog";
import { a2aConfig } from "./model";
import type { useA2aCard } from "./useA2aCard";
vi.mock("../../shared/api",()=>({api:vi.fn()}));vi.mock("../workbench/context",()=>({useWorkbench:vi.fn()}));
vi.mock("../../shared/ui",()=>({Editor:({value,onChange,label,readOnly}:{value:string;onChange?:(value:string)=>void;label:string;readOnly?:boolean})=><textarea aria-label={label} value={value} readOnly={readOnly} onChange={event=>onChange?.(event.target.value)}/>,Field:({label,children}:{label:string;children:React.ReactNode})=><label>{label}{children}</label>}));
afterEach(cleanup);
it("Stop invalidates a completed discovery candidate before it can attach",async()=>{
  vi.mocked(useWorkbench).mockReturnValue({request:{id:"request",url:"http://127.0.0.1/card.json",protocol:a2aConfig()},dark:false,authenticated:true,draft:{id:"w",data:{}},accountId:"owner"} as unknown as ReturnType<typeof useWorkbench>);
  const attach=vi.fn();const source={source:"",guard:()=>()=>true,context:()=>({workspace_id:"w"}),attach} as unknown as ReturnType<typeof useA2aCard>;
  let resolve!:(value:unknown)=>void;
  vi.mocked(api).mockImplementation(path=>path.endsWith("/cancel")?Promise.resolve({stopped:true}):new Promise(done=>{resolve=done;}));
  render(<A2aCardDialog open source={source} onOpenChange={vi.fn()}/>);
  fireEvent.click(screen.getByRole("button",{name:"发现 Agent Card"}));
  fireEvent.click(screen.getByRole("button",{name:"停止发现"}));
  await act(async()=>{resolve({specification:{id:"late"},card:{name:"late"},interfaces:[],warnings:[],dialect:"0.3"});});
  expect(attach).not.toHaveBeenCalled();
  expect(api).toHaveBeenCalledWith("/api/a2a/cards/discover/cancel","POST",expect.objectContaining({workspace_id:"w",discovery_id:expect.any(String)}));
});
it("saves a new request before starting owner-bound discovery",async()=>{
  const save=vi.fn().mockResolvedValue(true),attach=vi.fn();
  vi.mocked(api).mockReset();
  vi.mocked(useWorkbench).mockReturnValue({request:{id:"new",url:"http://127.0.0.1/card.json",protocol:a2aConfig()},dark:false,authenticated:true,draft:{id:"w",data:{}},accountId:"owner",dirty:true,save} as unknown as ReturnType<typeof useWorkbench>);
  const source={source:"",guard:()=>()=>true,context:()=>({workspace_id:"w"}),attach} as unknown as ReturnType<typeof useA2aCard>;
  vi.mocked(api).mockResolvedValue({specification:{id:"card"},card:{name:"ready"},interfaces:[],warnings:[],dialect:"0.3"});
  render(<A2aCardDialog open source={source} onOpenChange={vi.fn()}/>);
  await act(async()=>{fireEvent.click(screen.getByRole("button",{name:"发现 Agent Card"}));});
  expect(save).toHaveBeenCalledWith(true);
  expect(save.mock.invocationCallOrder[0]).toBeLessThan(vi.mocked(api).mock.invocationCallOrder[0]);
  expect(attach).toHaveBeenCalled();
});
