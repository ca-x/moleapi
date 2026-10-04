// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { api, saveFile } from "../../shared/api";
import { newRequest } from "../../shared/model";
import { useWorkbench } from "../workbench/context";
import GenerationDialog from "./GenerationDialog";
vi.mock("../../shared/api",()=>({api:vi.fn(),saveFile:vi.fn()}));
vi.mock("../workbench/context",()=>({useWorkbench:vi.fn()}));
vi.mock("../../shared/ui",()=>({
  Editor:({value,label}:{value:string;label:string})=><textarea aria-label={label} readOnly value={value}/>,
  Field:({label,children}:{label:string;children:React.ReactNode})=><label>{label}{children}</label>,
  Choice:({value,label,onChange,options,disabled}:{value:string;label:string;onChange:(value:string)=>void;options:{value:string;label:string}[];disabled:boolean})=><select aria-label={label} value={value} disabled={disabled} onChange={event=>onChange(event.target.value)}>{options.map(option=><option key={option.value} value={option.value}>{option.label}</option>)}</select>,
}));
const catalog={engine:"Scalar",targets:[{target:"shell",title:"Shell",clients:[{client:"curl",title:"cURL"}]}]};
const snippet={engine:"Scalar",target:"shell",client:"curl",code:"safe-code",warnings:[],include_secrets:false};
let state:ReturnType<typeof useWorkbench>;
beforeEach(()=>{
  vi.mocked(api).mockReset();vi.mocked(saveFile).mockReset();
  state={request:{...newRequest(),id:"r"},dark:false,authenticated:true,accountId:"owner",draft:{id:"w"},dirty:true,save:vi.fn().mockResolvedValue(true)} as unknown as ReturnType<typeof useWorkbench>;
  vi.mocked(useWorkbench).mockImplementation(()=>state);
});
afterEach(cleanup);
it("saves the draft before generating and exports only the selected result",async()=>{
  vi.mocked(api).mockImplementation(path=>Promise.resolve(path.endsWith("catalog")?catalog:snippet));
  render(<GenerationDialog open onOpenChange={vi.fn()}/>);
  await waitFor(()=>expect((screen.getByRole("button",{name:"生成"}) as HTMLButtonElement).disabled).toBe(false));
  await act(async()=>{fireEvent.click(screen.getByRole("button",{name:"生成"}));});
  expect(state.save).toHaveBeenCalledWith(true);
  const call=vi.mocked(api).mock.calls.findIndex(([path])=>path==="/api/generation/snippets");
  expect(vi.mocked(state.save).mock.invocationCallOrder[0]).toBeLessThan(vi.mocked(api).mock.invocationCallOrder[call]);
  expect(api).toHaveBeenCalledWith("/api/generation/snippets","POST",expect.objectContaining({workspace_id:"w",request_id:"r",include_secrets:false}));
  expect(screen.getByRole("textbox",{name:"生成的请求代码"})).toHaveProperty("value","safe-code");
  await act(async()=>{fireEvent.click(screen.getByRole("button",{name:"下载代码"}));});
  expect(saveFile).toHaveBeenCalledWith(expect.objectContaining({content:"safe-code",filename:"request-shell-curl.txt"}));
  fireEvent.click(screen.getByRole("checkbox"));
  expect(screen.queryByRole("textbox",{name:"生成的请求代码"})).toBeNull();
});
it("discards late generated credentials when the owning workspace changes",async()=>{
  let resolve!:(value:unknown)=>void;
  vi.mocked(api).mockImplementation(path=>path.endsWith("catalog")?Promise.resolve(catalog):new Promise(done=>{resolve=done;}));
  const view=render(<GenerationDialog open onOpenChange={vi.fn()}/>);
  await waitFor(()=>expect((screen.getByRole("button",{name:"生成"}) as HTMLButtonElement).disabled).toBe(false));
  await act(async()=>{fireEvent.click(screen.getByRole("button",{name:"生成"}));});
  state={...state,draft:{...state.draft!,id:"other"}};
  view.rerender(<GenerationDialog open onOpenChange={vi.fn()}/>);
  await act(async()=>{resolve({...snippet,code:"private-late-code",include_secrets:true});});
  expect(screen.queryByRole("textbox",{name:"生成的请求代码"})).toBeNull();
  expect(screen.queryByText("private-late-code")).toBeNull();
});
it("a save that reorders equivalent request keys does not discard generation",async()=>{
  vi.mocked(api).mockImplementation(path=>Promise.resolve(path.endsWith("catalog")?catalog:snippet));
  const view=render(<GenerationDialog open onOpenChange={vi.fn()}/>);
  state.save=vi.fn().mockImplementation(async()=>{
    state={...state,request:Object.fromEntries(Object.entries(state.request!).reverse()) as typeof state.request};
    view.rerender(<GenerationDialog open onOpenChange={vi.fn()}/>);
    return true;
  });
  await waitFor(()=>expect((screen.getByRole("button",{name:"生成"}) as HTMLButtonElement).disabled).toBe(false));
  await act(async()=>{fireEvent.click(screen.getByRole("button",{name:"生成"}));});
  expect(api).toHaveBeenCalledWith("/api/generation/snippets","POST",expect.objectContaining({request_id:"r"}));
  expect(screen.getByRole("textbox",{name:"生成的请求代码"})).toHaveProperty("value","safe-code");
});
