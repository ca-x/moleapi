// @vitest-environment jsdom
import {act,cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {useWorkbench} from "../workbench/context";
import DataWorkbench from "./DataWorkbench";
import {dataConfig} from "./model";
vi.mock("../workbench/context",()=>({useWorkbench:vi.fn()}));
vi.mock("./DataSourceFields",()=>({DataSourceFields:()=>null}));
vi.mock("./DataResults",()=>({DataResults:()=>null}));
vi.mock("./DataSqlEditor",()=>({DataSqlEditor:()=>null}));
afterEach(cleanup);
function state() {return {authenticated:true,accountId:"first",selectedId:"workspace",draft:{id:"workspace",data:{active_environment_id:"dev"}},request:{id:"request",protocol:dataConfig()},dark:false,updateRequest:vi.fn(),dataRun:{current:null as (()=>void)|null},protocolSession:{session:{id:"session",state:"open"},events:[],send:vi.fn<(_message:unknown)=>Promise<boolean>>().mockResolvedValue(true),connect:vi.fn(),close:vi.fn(),busy:false,sending:false,error:"",dropped:0}};}
it("a late rejected admission cannot clear the new scope's active query",async()=>{
  const value=state();const pending:((ok:boolean)=>void)[]=[];
  value.protocolSession.send.mockImplementation(()=>new Promise(done=>pending.push(done)));
  vi.mocked(useWorkbench).mockImplementation(()=>value as unknown as ReturnType<typeof useWorkbench>);
  const view=render(<DataWorkbench/>);
  fireEvent.click(screen.getByRole("button",{name:"运行当前查询"}));
  value.accountId="second";value.protocolSession.session.id="second-session";view.rerender(<DataWorkbench/>);
  fireEvent.click(screen.getByRole("button",{name:"运行当前查询"}));
  await act(async()=>{pending[0](false);});
  expect((screen.getByRole("button",{name:"取消查询"}) as HTMLButtonElement).disabled).toBe(false);
  expect(value.protocolSession.send).toHaveBeenCalledTimes(2);
});
it("the registered keyboard dispatcher runs selected SQL on an open connection",async()=>{
  const value=state();value.request.protocol.sql="SELECT ';' AS first; SELECT 2 AS second;";
  vi.mocked(useWorkbench).mockReturnValue(value as unknown as ReturnType<typeof useWorkbench>);
  render(<DataWorkbench/>);
  await act(async()=>{value.dataRun.current?.();});
  expect(value.protocolSession.send).toHaveBeenCalledWith(expect.objectContaining({kind:"data_query",sql:"SELECT ';' AS first;",read_only:true}));
  expect(value.protocolSession.connect).not.toHaveBeenCalled();
});
it("the dispatcher connects before any SQL is admitted",()=>{
  const value=state();value.protocolSession.session.state="closed";
  vi.mocked(useWorkbench).mockReturnValue(value as unknown as ReturnType<typeof useWorkbench>);
  render(<DataWorkbench/>);act(()=>value.dataRun.current?.());
  expect(value.protocolSession.connect).toHaveBeenCalledTimes(1);
  expect(value.protocolSession.send).not.toHaveBeenCalled();
});

it("repeated dispatch before React renders admits only one query",()=>{
  const value=state();value.protocolSession.send.mockReturnValue(new Promise(()=>{}));
  vi.mocked(useWorkbench).mockReturnValue(value as unknown as ReturnType<typeof useWorkbench>);
  render(<DataWorkbench/>);
  act(()=>{value.dataRun.current?.();value.dataRun.current?.();});
  expect(value.protocolSession.send).toHaveBeenCalledTimes(1);
});
