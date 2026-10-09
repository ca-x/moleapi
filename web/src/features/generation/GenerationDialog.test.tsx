// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { api } from "../../shared/api";
import { newRequest } from "../../shared/model";
import { useWorkbench } from "../workbench/context";
import GenerationDialog from "./GenerationDialog";
vi.mock("../../shared/api",()=>({api:vi.fn()}));
vi.mock("./saveProjectFile",()=>({saveProjectFile:vi.fn()}));
import {saveProjectFile} from "./saveProjectFile";
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
  vi.mocked(api).mockReset();vi.mocked(saveProjectFile).mockReset();
  state={request:{...newRequest(),id:"r"},dark:false,authenticated:true,accountId:"owner",draft:{id:"w"},dirty:true,save:vi.fn().mockResolvedValue(true)} as unknown as ReturnType<typeof useWorkbench>;
  vi.mocked(useWorkbench).mockImplementation(()=>state);
});
afterEach(cleanup);
it.each([["go","native","go"],["csharp","httpclient","cs"],["shell","curl_windows","cmd"],["node","native","cjs"]])("downloads %s request code with its source extension and rejects a later owner boundary",async(target,client,extension)=>{
  const targets={...catalog,targets:[...catalog.targets.filter(row=>row.target!==target),{target,title:target,clients:[{client,title:client}]}]};
  const result={...snippet,target,client,code:`generated-${target}`};
  vi.mocked(api).mockImplementation(path=>Promise.resolve(path.endsWith("catalog")?targets:result));
  const view=render(<GenerationDialog open onOpenChange={vi.fn()}/>);
  await waitFor(()=>expect((screen.getByRole("button",{name:"生成"}) as HTMLButtonElement).disabled).toBe(false));
  fireEvent.change(screen.getByLabelText("代码语言"),{target:{value:target}});
  await act(async()=>{fireEvent.click(screen.getByRole("button",{name:"生成"}));});
  await act(async()=>{fireEvent.click(screen.getByRole("button",{name:"下载代码"}));});
  expect(saveProjectFile).toHaveBeenCalledWith(`request-${target}-${client}.${extension}`,new TextEncoder().encode(`generated-${target}`),expect.any(Function));
  const guard=vi.mocked(saveProjectFile).mock.calls[0][2];expect(guard()).toBe(true);
  state.accountId="another-owner";view.rerender(<GenerationDialog open onOpenChange={vi.fn()}/>);expect(guard()).toBe(false);
});
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
  expect(saveProjectFile).toHaveBeenCalledWith("request-shell-curl.sh",new TextEncoder().encode("safe-code"),expect.any(Function));
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
it("switches an existing catalog-load failure without refetching or changing the request", async () => {
  const { setLanguage } = await import("../../shared/i18n");
  vi.mocked(api).mockRejectedValue(new Error("upstream unreachable"));
  const original = JSON.stringify(state.request);
  render(<GenerationDialog open onOpenChange={vi.fn()} />);
  await waitFor(() => expect(screen.getByRole("alert").textContent).toBe("无法读取生成器列表，请关闭后重试。"));
  await act(async () => { await setLanguage("en"); });
  expect(screen.getByRole("alert").textContent).toBe("Unable to load generators. Close this dialog and try again.");
  expect(api).toHaveBeenCalledTimes(1);
  expect(JSON.stringify(state.request)).toBe(original);
});
it("preserves an exact upstream generation error when language changes", async () => {
  const { setLanguage } = await import("../../shared/i18n");
  const upstream = "请求失败 (503) · upstream 原文";
  vi.mocked(api).mockImplementation(path => path.endsWith("catalog") ? Promise.resolve(catalog) : Promise.reject(new Error(upstream)));
  render(<GenerationDialog open onOpenChange={vi.fn()} />);
  await waitFor(() => expect((screen.getByRole("button", { name: "生成" }) as HTMLButtonElement).disabled).toBe(false));
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "生成" })); });
  expect(screen.getByRole("alert").textContent).toBe(upstream);
  await act(async () => { await setLanguage("en"); });
  expect(screen.getByRole("alert").textContent).toBe(upstream);
  expect(api).toHaveBeenCalledTimes(2);
});
