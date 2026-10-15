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
it("offers both Dart file adapters with async and package prerequisites",async()=>{
 state.request={...state.request!,body_kind:"binary",body:JSON.stringify({file_name:"upload.bin",mime:"application/octet-stream",base64:"AP+AClg="})};
 const choices={...catalog,targets:[{target:"dart",title:"Dart",clients:[{client:"http",title:"Http",binary_file:true},{client:"dio",title:"Dio",binary_file:true}]}]};
 vi.mocked(api).mockImplementation(path=>Promise.resolve(path.endsWith("catalog")?choices:{...snippet,target:"dart",client:"dio",code:"original-dart-file-reader"}));
 render(<GenerationDialog open onOpenChange={vi.fn()}/>);await screen.findByRole("option",{name:"Dart"});
 fireEvent.change(screen.getByLabelText("代码语言"),{target:{value:"dart"}});
 fireEvent.change(screen.getByLabelText("代码 HTTP 库"),{target:{value:"dio"}});
 fireEvent.click(screen.getByRole("button",{name:"生成"}));
 expect(await screen.findByText("Dart 示例需放入 async 方法，并安装所选 http／dio 及 http_parser 依赖。文件示例需要 dart:io，编码转换需要 dart:convert。")).toBeTruthy();
 expect(screen.getByDisplayValue("original-dart-file-reader")).toBeTruthy();
});
it("shows Kotlin dependency and extension imports after generating a file reader",async()=>{
 state.request={...state.request!,body_kind:"binary",body:JSON.stringify({file_name:"upload.bin",mime:"application/octet-stream",base64:"AP+AClg="})};
 const choices={...catalog,targets:[{target:"kotlin",title:"Kotlin",clients:[{client:"okhttp",title:"OkHttp",binary_file:true}]}]};
 vi.mocked(api).mockImplementation(path=>Promise.resolve(path.endsWith("catalog")?choices:{...snippet,target:"kotlin",client:"okhttp",code:"original-kotlin-file-reader"}));
 render(<GenerationDialog open onOpenChange={vi.fn()}/>);await screen.findByRole("option",{name:"Kotlin"});
 fireEvent.change(screen.getByLabelText("代码语言"),{target:{value:"kotlin"}});
 fireEvent.click(screen.getByRole("button",{name:"生成"}));
 expect(await screen.findByText("Kotlin 代码需放入方法体，并安装 OkHttp 依赖。请导入 asRequestBody、toRequestBody、toMediaTypeOrNull 扩展方法；文件示例还需要 java.io.File。")).toBeTruthy();
 expect(screen.getByDisplayValue("original-kotlin-file-reader")).toBeTruthy();
});
it("offers verified Java file readers and explains native snippet prerequisites",async()=>{
 state.request={...state.request!,body_kind:"binary",body:JSON.stringify({file_name:"upload.bin",mime:"application/octet-stream",base64:"AP+AClg="})};
 const clients=["nethttp","okhttp","asynchttp","unirest"].map(client=>({client,title:client,binary_file:true}));
 const choices={...catalog,targets:[{target:"java",title:"Java",clients}]};
 vi.mocked(api).mockImplementation(path=>Promise.resolve(path.endsWith("catalog")?choices:{...snippet,target:"java",client:"unirest",code:"original-java-file-reader"}));
 render(<GenerationDialog open onOpenChange={vi.fn()}/>);
 await screen.findByRole("option",{name:"Java"});
 fireEvent.change(screen.getByLabelText("代码语言"),{target:{value:"java"}});
 for(const client of clients)expect(screen.getByRole("option",{name:client.title})).toBeTruthy();
 fireEvent.change(screen.getByLabelText("代码 HTTP 库"),{target:{value:"unirest"}});
 fireEvent.click(screen.getByRole("button",{name:"生成"}));
 await waitFor(()=>expect(api).toHaveBeenCalledWith("/api/generation/snippets","POST",expect.objectContaining({target:"java",client:"unirest",include_secrets:false})));
 expect(await screen.findByText("Java 代码需放入方法或 main，并补充所选 HTTP 库的 import 与 Maven／Gradle 依赖；java.net.http 使用 JDK 自带客户端。")).toBeTruthy();
 expect(screen.getByDisplayValue("original-java-file-reader")).toBeTruthy();
});
it("offers binary-capable libraries and excludes file-placeholder adapters for raw file bodies",async()=>{
 state.request={...state.request!,body_kind:"binary",body:JSON.stringify({file_name:"upload.bin",mime:"application/octet-stream",base64:"AP+AClg="})};
 const choices={...catalog,targets:[{target:"shell",title:"Shell",clients:[{client:"curl",title:"cURL",binary_file:true},{client:"missing",title:"Placeholder library",binary_file:false}]},{target:"java",title:"Java",clients:[{client:"unsupported",title:"Placeholder reader",binary_file:false}]}]};
 vi.mocked(api).mockImplementation(path=>Promise.resolve(path.endsWith("catalog")?choices:snippet));
 render(<GenerationDialog open onOpenChange={vi.fn()}/>);await screen.findByText("当前显示可读取原始文件字节的适配器；其他正文类型仍可选择完整语言与库列表。");
 expect(screen.queryByRole("option",{name:"Java"})).toBeNull();expect(screen.queryByRole("option",{name:"Placeholder library"})).toBeNull();expect(screen.getByRole("option",{name:"cURL"})).toBeTruthy();
 fireEvent.click(screen.getByRole("button",{name:"生成"}));await waitFor(()=>expect(api).toHaveBeenCalledWith("/api/generation/snippets","POST",expect.objectContaining({target:"shell",client:"curl"})));
});
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
