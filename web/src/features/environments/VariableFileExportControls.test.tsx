// @vitest-environment jsdom
import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,expect,it,vi} from "vitest";
import {api,saveFile} from "../../shared/api";
import {initialData} from "../../shared/model";
import {useWorkbench} from "../workbench/context";
import VariableFileExportControls from "./VariableFileExportControls";

vi.mock("../../shared/api",async original=>({...await original<typeof import("../../shared/api")>(),api:vi.fn(),saveFile:vi.fn()}));
vi.mock("../workbench/context",()=>({useWorkbench:vi.fn()}));
afterEach(()=>{cleanup();vi.mocked(api).mockReset();vi.mocked(saveFile).mockReset();vi.mocked(useWorkbench).mockReset();});
function state(){return {accountId:"owner",draft:{id:"w",revision:3,data:initialData()},environment:{id:"dev"},saving:false,dirty:false,save:vi.fn(),stateRef:{current:{draft:null,saved:null}}};}
it("exports an owned environment privately and requires an explicit switch for full globals",async()=>{
  const value=state();vi.mocked(useWorkbench).mockReturnValue(value as unknown as ReturnType<typeof useWorkbench>);
  const file={filename:"env.json",content:"{}",mime:"application/json"};vi.mocked(api).mockResolvedValue(file);
  render(<Theme><VariableFileExportControls/></Theme>);
  fireEvent.click(screen.getByRole("button",{name:"导出当前环境"}));
  await waitFor(()=>expect(saveFile).toHaveBeenCalledWith(file));
  expect(api).toHaveBeenCalledWith("/api/workspaces/w/variables/export","POST",{scope:"environment",environment_id:"dev",include_secrets:false,expected_revision:3});
  fireEvent.click(screen.getByRole("checkbox",{name:"包含私密变量值"}));
  fireEvent.click(screen.getByRole("button",{name:"导出项目全局变量"}));
  await waitFor(()=>expect(api).toHaveBeenLastCalledWith("/api/workspaces/w/variables/export","POST",{scope:"globals",environment_id:null,include_secrets:true,expected_revision:3}));
});
it("does not download a late export after an account switch",async()=>{
  const value=state();vi.mocked(useWorkbench).mockReturnValue(value as unknown as ReturnType<typeof useWorkbench>);
  let resolve!:(result:unknown)=>void;vi.mocked(api).mockReturnValue(new Promise(done=>{resolve=done;}));
  const view=render(<Theme><VariableFileExportControls/></Theme>);
  fireEvent.click(screen.getByRole("button",{name:"导出当前环境"}));
  value.accountId="other";view.rerender(<Theme><VariableFileExportControls/></Theme>);
  await act(async()=>resolve({filename:"late.json",content:"secret",mime:"application/json"}));
  expect(saveFile).not.toHaveBeenCalled();
});
it("does not download a late full export after switching away and back",async()=>{
  const value=state();vi.mocked(useWorkbench).mockReturnValue(value as unknown as ReturnType<typeof useWorkbench>);
  let resolve!:(result:unknown)=>void;vi.mocked(api).mockReturnValue(new Promise(done=>{resolve=done;}));
  const view=render(<Theme><VariableFileExportControls/></Theme>);
  fireEvent.click(screen.getByRole("checkbox",{name:"包含私密变量值"}));
  fireEvent.click(screen.getByRole("button",{name:"导出当前环境"}));
  const original=value.draft;
  value.draft={id:"other",revision:3,data:initialData()};
  view.rerender(<Theme><VariableFileExportControls/></Theme>);
  value.draft=original;
  view.rerender(<Theme><VariableFileExportControls/></Theme>);
  await act(async()=>resolve({filename:"private.json",content:"secret",mime:"application/json"}));
  expect(saveFile).not.toHaveBeenCalled();
  expect(screen.getByRole("checkbox",{name:"包含私密变量值"}).getAttribute("aria-checked")).toBe("false");
});
it("resets full export when switching workspaces",()=>{
  const value=state();vi.mocked(useWorkbench).mockReturnValue(value as unknown as ReturnType<typeof useWorkbench>);
  const view=render(<Theme><VariableFileExportControls/></Theme>);
  fireEvent.click(screen.getByRole("checkbox",{name:"包含私密变量值"}));
  expect(screen.getByRole("checkbox",{name:"包含私密变量值"}).getAttribute("aria-checked")).toBe("true");
  value.draft.id="other";view.rerender(<Theme><VariableFileExportControls/></Theme>);
  expect(screen.getByRole("checkbox",{name:"包含私密变量值"}).getAttribute("aria-checked")).toBe("false");
});
