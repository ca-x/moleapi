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
