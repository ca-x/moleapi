// @vitest-environment jsdom
import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,expect,it,vi} from "vitest";
import {api} from "../../shared/api";
import {initialData} from "../../shared/model";
import {useWorkbench} from "../workbench/context";
import VariableFileImportDialog from "./VariableFileImportDialog";

vi.mock("../../shared/api",async original=>({...await original<typeof import("../../shared/api")>(),api:vi.fn()}));
vi.mock("../workbench/context",()=>({useWorkbench:vi.fn()}));
afterEach(()=>{cleanup();vi.mocked(api).mockReset();vi.mocked(useWorkbench).mockReset();});
function state(){const draft={id:"w",revision:1,data:initialData()};return {accountId:"owner",draft,saving:false,
  updateData:vi.fn((update:(data:typeof draft.data)=>typeof draft.data)=>{draft.data=update(draft.data);}),
  save:vi.fn(async()=>structuredClone(draft))};}
function show(value:ReturnType<typeof state>,onOpenChange=vi.fn()){
  vi.mocked(useWorkbench).mockReturnValue(value as unknown as ReturnType<typeof useWorkbench>);
  const view=render(<Theme><VariableFileImportDialog open onOpenChange={onOpenChange}/></Theme>);
  fireEvent.change(screen.getByRole("textbox",{name:"Postman 变量文件内容"}),{target:{value:"fixture"}});
  fireEvent.click(screen.getByRole("button",{name:"预览变量文件"}));
  return {view,onOpenChange};
}
it("previews and CAS-saves an imported private environment without replacing existing rows",async()=>{
  const value=state();const row={id:"p",key:"token",value:"private",enabled:true,secret:true};
  vi.mocked(api).mockResolvedValue({scope:"environment",name:"Imported",variables:[row],warnings:[]});
  const {onOpenChange}=show(value);
  await waitFor(()=>expect(screen.getByRole("textbox",{name:"导入环境名称"})).toHaveProperty("value","Imported"));
  fireEvent.click(screen.getByRole("button",{name:"新建导入环境"}));
  await waitFor(()=>expect(value.save).toHaveBeenCalledWith(true));
  expect(api).toHaveBeenCalledWith("/api/workspaces/w/variables/import-preview","POST",{content:"fixture",expected_revision:1});
  expect(value.draft.data.environments.some(item=>item.name==="Imported"&&item.variables[0].secret)).toBe(true);
  expect(onOpenChange).toHaveBeenCalledWith(false);
});
it("shows the replacement count before applying a globals file",async()=>{
  const value=state();value.draft.data.global_variables=[{id:"old",key:"old",value:"keep until confirm",enabled:true}];
  const row={id:"new",key:"region",value:"东京",enabled:true,secret:false};
  vi.mocked(api).mockResolvedValue({scope:"globals",name:"Globals",variables:[row],warnings:[]});
  show(value);
  await waitFor(()=>expect(screen.getByText("将替换当前 1 个项目全局变量，导入 1 个。")).toBeTruthy());
  expect(value.updateData).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button",{name:"替换项目全局变量"}));
  await waitFor(()=>expect(value.draft.data.global_variables).toEqual([row]));
});
it("invalidates a late preview when the workspace changes but retains the file in the same account",async()=>{
  const value=state();let resolve!:(result:unknown)=>void;
  vi.mocked(api).mockReturnValue(new Promise(done=>{resolve=done;}));
  const {view}=show(value);
  value.draft={id:"other",revision:1,data:initialData()};
  view.rerender(<Theme><VariableFileImportDialog open onOpenChange={vi.fn()}/></Theme>);
  await act(async()=>resolve({scope:"environment",name:"Late",variables:[],warnings:[]}));
  expect(value.updateData).not.toHaveBeenCalled();
  expect((screen.getByRole("textbox",{name:"Postman 变量文件内容"}) as HTMLTextAreaElement).value).toBe("fixture");
  expect(screen.getByRole("alert").textContent).toContain("工作区已切换");
});
it("clears the pasted file when the account changes",async()=>{
  const value=state();vi.mocked(api).mockResolvedValue({scope:"environment",name:"Imported",variables:[],warnings:[]});
  const {view}=show(value);
  await waitFor(()=>expect(screen.getByRole("textbox",{name:"导入环境名称"})).toBeTruthy());
  value.accountId="other";view.rerender(<Theme><VariableFileImportDialog open onOpenChange={vi.fn()}/></Theme>);
  expect((screen.getByRole("textbox",{name:"Postman 变量文件内容"}) as HTMLTextAreaElement).value).toBe("");
  expect(screen.queryByRole("textbox",{name:"导入环境名称"})).toBeNull();
});
it("reuses one environment ID when a failed save is retried after a revision change",async()=>{
  const value=state();value.save.mockResolvedValueOnce(null as never);
  const baseline=value.draft.data.environments.length;
  vi.mocked(api).mockResolvedValue({scope:"environment",name:"Imported",variables:[],warnings:[]});
  const {view}=show(value);
  await waitFor(()=>expect(screen.getByRole("button",{name:"新建导入环境"}).hasAttribute("disabled")).toBe(false));
  fireEvent.click(screen.getByRole("button",{name:"新建导入环境"}));
  await waitFor(()=>expect(screen.getByRole("alert").textContent).toContain("未写入工作区"));
  expect(value.draft.data.environments).toHaveLength(baseline+1);
  value.draft.revision=2;
  view.rerender(<Theme><VariableFileImportDialog open onOpenChange={vi.fn()}/></Theme>);
  fireEvent.click(screen.getByRole("button",{name:"新建导入环境"}));
  expect(screen.getByRole("alert").textContent).toContain("重新预览");
  fireEvent.click(screen.getByRole("button",{name:"预览变量文件"}));
  await waitFor(()=>expect(screen.getByRole("button",{name:"新建导入环境"}).hasAttribute("disabled")).toBe(false));
  fireEvent.click(screen.getByRole("button",{name:"新建导入环境"}));
  await waitFor(()=>expect(value.save).toHaveBeenCalledTimes(2));
  expect(value.draft.data.environments).toHaveLength(baseline+1);
});
