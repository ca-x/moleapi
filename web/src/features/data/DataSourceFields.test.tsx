// @vitest-environment jsdom
import {useState} from "react";
import {act,cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {DataSourceFields} from "./DataSourceFields";
import type {DataConfig} from "./types";
import {dataConfig} from "./model";
import {pickDataFile} from "./pickFile";
vi.mock("./pickFile",()=>({pickDataFile:vi.fn()}));
vi.mock("../../shared/ui",()=>({Choice:()=>null,Field:({label,children}:{label:string;children:React.ReactNode})=><label>{label}{children}</label>}));
afterEach(()=>{cleanup();vi.clearAllMocks();});
it("committing a deferred file selection preserves edits made while reading",async()=>{
  let finish!:(value:{name:string;base64:string})=>void;
  vi.mocked(pickDataFile).mockReturnValue(new Promise(done=>{finish=done;}));
  function Harness(){const[config,setConfig]=useState<DataConfig>({...dataConfig(),source:"local_file" as const});return <><input aria-label="SQL document" value={config.sql} onChange={e=>setConfig({...config,sql:e.target.value})}/><DataSourceFields config={config} change={patch=>setConfig({...config,...patch})} connected={false}/></>;}
  render(<Harness/>);fireEvent.click(screen.getByRole("button",{name:"选择数据文件"}));
  fireEvent.change(screen.getByRole("textbox",{name:"SQL document"}),{target:{value:"SELECT updated FROM data"}});
  await act(async()=>{finish({name:"new.csv",base64:"MQ=="});});
  expect((screen.getByRole("textbox",{name:"SQL document"}) as HTMLInputElement).value).toBe("SELECT updated FROM data");
  expect(screen.getByText("new.csv")).toBeTruthy();
});
it("a selection finishing after Connect cannot relabel the active dataset",async()=>{
  let finish!:(value:{name:string;base64:string})=>void;
  vi.mocked(pickDataFile).mockReturnValue(new Promise(done=>{finish=done;}));
  const config={...dataConfig(),source:"local_file" as const,file_name:"original.csv"};const change=vi.fn();
  const view=render(<DataSourceFields config={config} change={change} connected={false}/>);
  fireEvent.click(screen.getByRole("button",{name:"选择数据文件"}));
  view.rerender(<DataSourceFields config={config} change={change} connected={true}/>);
  await act(async()=>{finish({name:"new.csv",base64:"MQ=="});});
  expect(change).not.toHaveBeenCalled();expect(screen.getByText("original.csv")).toBeTruthy();
});
it("selecting a new XLSX workbook resets a previous worksheet name",async()=>{
  vi.mocked(pickDataFile).mockResolvedValue({name:"transactions.xlsx",base64:"UEs="});
  const config={...dataConfig(),source:"local_file" as const,file_format:"xlsx" as const,sheet_name:"Old Sheet"};
  const change=vi.fn();
  render(<DataSourceFields config={config} change={change} connected={false}/>);
  await act(async()=>{fireEvent.click(screen.getByRole("button",{name:"选择数据文件"}));});
  expect(change).toHaveBeenCalledWith(expect.objectContaining({
    file_name:"transactions.xlsx",file_format:"xlsx",sheet_name:""
  }));
});
it("infers ODS from a selected file and exposes worksheet and cached-value controls",async()=>{
  vi.mocked(pickDataFile).mockResolvedValue({name:"transactions.ods",base64:"UEs="});
  const config={...dataConfig(),source:"local_file" as const,file_format:"ods" as const,sheet_name:"Transactions"};
  const change=vi.fn();
  render(<DataSourceFields config={config} change={change} connected={false}/>);
  expect(screen.getByText("ODS 只读取缓存值，不计算公式；超出精确范围的整数请存为文本。")).toBeTruthy();
  expect(screen.getByRole("textbox",{name:"工作表名称"})).toBeTruthy();
  expect(screen.getByText("首行为列名")).toBeTruthy();
  await act(async()=>{fireEvent.click(screen.getByRole("button",{name:"选择数据文件"}));});
  expect(change).toHaveBeenCalledWith(expect.objectContaining({file_name:"transactions.ods",file_format:"ods",sheet_name:""}));
});
it("infers XLS from a selected file and exposes worksheet and cached-value controls",async()=>{
  vi.mocked(pickDataFile).mockResolvedValue({name:"transactions.xls",base64:"0M8="});
  const config={...dataConfig(),source:"local_file" as const,file_format:"xls" as const,sheet_name:"Transactions"};
  const change=vi.fn();
  render(<DataSourceFields config={config} change={change} connected={false}/>);
  expect(screen.getByText("XLS 只读取缓存值，不计算公式；超出精确范围的整数请存为文本。")).toBeTruthy();
  expect(screen.getByRole("textbox",{name:"工作表名称"})).toBeTruthy();
  expect(screen.getByText("首行为列名")).toBeTruthy();
  await act(async()=>{fireEvent.click(screen.getByRole("button",{name:"选择数据文件"}));});
  expect(change).toHaveBeenCalledWith(expect.objectContaining({file_name:"transactions.xls",file_format:"xls",sheet_name:""}));
});
