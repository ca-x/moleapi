// @vitest-environment jsdom
import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {api} from "../../shared/api";
import {initialData} from "../../shared/model";
import {useWorkbench} from "../workbench/context";
import DataResultDatasetDialog from "./DataResultDatasetDialog";
import type {Result} from "./types";

vi.mock("../../shared/api",async original=>({...await original<typeof import("../../shared/api")>(),api:vi.fn()}));
vi.mock("../workbench/context",()=>({useWorkbench:vi.fn()}));
afterEach(()=>{cleanup();vi.mocked(api).mockReset();});
const result:Result={id:"query",columns:[{name:"exact",data_type:"BIGINT",nullable:false}],rows:[[{kind:"integer",value:"9007199254740993"}]],elapsed_ms:1,rows_affected:0,truncated:false,limit_reason:null,done:true};
function state(){const draft={id:"w",revision:1,data:initialData()};return {accountId:"owner",draft,request:{id:"r"},saving:false,updateData:vi.fn((update:(data:typeof draft.data)=>typeof draft.data)=>{draft.data=update(draft.data);}),save:vi.fn(async()=>structuredClone(draft))};}

it("previews and CAS-saves a private Data snapshot without saving SQL metadata",async()=>{
  const value=state();vi.mocked(useWorkbench).mockReturnValue(value as unknown as ReturnType<typeof useWorkbench>);
  vi.mocked(api).mockResolvedValue({rows:1});
  const onOpenChange=vi.fn();render(<DataResultDatasetDialog open onOpenChange={onOpenChange} result={result} scope="owner/w/r/query"/>);
  fireEvent.change(screen.getByRole("textbox",{name:"数据集名称"}),{target:{value:"Exact rows"}});
  fireEvent.click(screen.getByRole("button",{name:"保存数据集"}));
  await waitFor(()=>expect(value.save).toHaveBeenCalledWith(true));
  expect(api).toHaveBeenCalledWith("/api/testing/dataset/preview","POST",expect.objectContaining({workspace_id:"w",dataset:{format:"json",source:'[{"exact":"9007199254740993"}]'}}));
  const update=value.updateData.mock.calls[0][0] as (data:ReturnType<typeof initialData>)=>ReturnType<typeof initialData>;
  const saved=update(value.draft.data).datasets?.[0];
  expect(saved).toEqual(expect.objectContaining({name:"Exact rows",secret:true,source:{format:"json",source:'[{"exact":"9007199254740993"}]'}}));
  expect(JSON.stringify(saved)).not.toContain("query");
  expect(onOpenChange).toHaveBeenCalledWith(false);
});
it("drops a late preview after the account changes",async()=>{
  const value=state();vi.mocked(useWorkbench).mockReturnValue(value as unknown as ReturnType<typeof useWorkbench>);
  let resolve!: (value:unknown)=>void;vi.mocked(api).mockReturnValue(new Promise(done=>{resolve=done;}));
  const view=render(<DataResultDatasetDialog open onOpenChange={vi.fn()} result={result} scope="owner/w/r/query"/>);
  fireEvent.change(screen.getByRole("textbox",{name:"数据集名称"}),{target:{value:"Rows"}});
  fireEvent.click(screen.getByRole("button",{name:"保存数据集"}));
  value.accountId="other";view.rerender(<DataResultDatasetDialog open onOpenChange={vi.fn()} result={result} scope="other/w/r/query"/>);
  await act(async()=>resolve({rows:1}));
  expect(value.updateData).not.toHaveBeenCalled();
  expect((screen.getByRole("textbox",{name:"数据集名称"}) as HTMLInputElement).value).toBe("Rows");
  expect(screen.getByRole("alert").textContent).toContain("工作区");
});
it("does not claim success from a concurrent save of an older workspace snapshot",async()=>{
  const value=state();value.saving=true;
  vi.mocked(useWorkbench).mockReturnValue(value as unknown as ReturnType<typeof useWorkbench>);
  vi.mocked(api).mockResolvedValue({rows:1});
  const onOpenChange=vi.fn();render(<DataResultDatasetDialog open onOpenChange={onOpenChange} result={result} scope="owner/w/r/query"/>);
  fireEvent.change(screen.getByRole("textbox",{name:"数据集名称"}),{target:{value:"Rows"}});
  fireEvent.click(screen.getByRole("button",{name:"保存数据集"}));
  await waitFor(()=>expect(screen.getByRole("alert").textContent).toContain("保存"));
  expect(value.save).not.toHaveBeenCalled();
  expect(value.updateData).not.toHaveBeenCalled();
  expect(onOpenChange).not.toHaveBeenCalledWith(false);
});
it("keeps the name and reports a conflict if save returns an older snapshot",async()=>{
  const value=state();const old=structuredClone(value.draft);
  value.save.mockResolvedValue(old);
  vi.mocked(useWorkbench).mockReturnValue(value as unknown as ReturnType<typeof useWorkbench>);
  vi.mocked(api).mockResolvedValue({rows:1});
  const onOpenChange=vi.fn();render(<DataResultDatasetDialog open onOpenChange={onOpenChange} result={result} scope="owner/w/r/query"/>);
  fireEvent.change(screen.getByRole("textbox",{name:"数据集名称"}),{target:{value:"Rows"}});
  fireEvent.click(screen.getByRole("button",{name:"保存数据集"}));
  await waitFor(()=>expect(screen.getByRole("alert").textContent).toContain("未写入工作区"));
  expect((screen.getByRole("textbox",{name:"数据集名称"}) as HTMLInputElement).value).toBe("Rows");
  expect(onOpenChange).not.toHaveBeenCalledWith(false);
});
it("keeps oversized results out of the workspace and gives a specific limit",async()=>{
  const value=state();vi.mocked(useWorkbench).mockReturnValue(value as unknown as ReturnType<typeof useWorkbench>);
  const many={...result,rows:Array.from({length:101},()=>[{kind:"integer" as const,value:"1"}])};
  render(<DataResultDatasetDialog open onOpenChange={vi.fn()} result={many} scope="owner/w/r/many"/>);
  fireEvent.change(screen.getByRole("textbox",{name:"数据集名称"}),{target:{value:"Too many"}});
  fireEvent.click(screen.getByRole("button",{name:"保存数据集"}));
  await waitFor(()=>expect(screen.getByRole("alert").textContent).toContain("100"));
  expect(api).not.toHaveBeenCalled();
  expect(value.updateData).not.toHaveBeenCalled();
});
it("clears a previous query's name when the result scope changes",()=>{
  const value=state();vi.mocked(useWorkbench).mockReturnValue(value as unknown as ReturnType<typeof useWorkbench>);
  const view=render(<DataResultDatasetDialog open onOpenChange={vi.fn()} result={result} scope="owner/w/r/old"/>);
  fireEvent.change(screen.getByRole("textbox",{name:"数据集名称"}),{target:{value:"Old query"}});
  view.rerender(<DataResultDatasetDialog open onOpenChange={vi.fn()} result={{...result,id:"new"}} scope="owner/w/r/new"/>);
  expect((screen.getByRole("textbox",{name:"数据集名称"}) as HTMLInputElement).value).toBe("");
});
