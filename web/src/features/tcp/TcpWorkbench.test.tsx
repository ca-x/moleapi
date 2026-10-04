// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useWorkbench } from "../workbench/context";
import TcpWorkbench from "./TcpWorkbench";
import { tcpConfig } from "./model";
vi.mock("../workbench/context",()=>({useWorkbench:vi.fn()}));
vi.mock("../protocols/SessionEventPane",()=>({default:()=> <div>event fixture</div>}));
vi.mock("../../shared/ui",()=>({Choice:({value,label}:{value:string;label:string})=><input readOnly value={value} aria-label={label}/>,Editor:({value,label}:{value:string;label:string})=><textarea readOnly value={value} aria-label={label}/>,Field:({label,children}:{label:string;children:React.ReactNode})=><label>{label}{children}</label>}));
let state:ReturnType<typeof useWorkbench>;
beforeEach(()=>{state={authenticated:true,accountId:"owner",draft:{id:"w",data:{active_environment_id:null}},request:{id:"r",protocol:tcpConfig()},dark:false,protocolSession:{session:{id:"s",state:"open",client_half_closed:false,received_bytes:0,sent_bytes:0},events:[],send:vi.fn(),close:vi.fn(),busy:false,sending:false,dropped:0,error:""},setGuard:vi.fn(),updateRequest:vi.fn()} as unknown as ReturnType<typeof useWorkbench>;vi.mocked(useWorkbench).mockImplementation(()=>state);});
afterEach(cleanup);
it("a late send cannot move the new owner's TCP pane to results",async()=>{
  let done!:(value:boolean)=>void;vi.mocked(state.protocolSession.send).mockReturnValue(new Promise(resolve=>{done=resolve;}));
  const view=render(<TcpWorkbench/>);fireEvent.click(screen.getByRole("button",{name:"发送报文"}));
  state={...state,accountId:"other",request:{...state.request!,id:"other"}};view.rerender(<TcpWorkbench/>);
  await act(async()=>{done(true);});expect(screen.getByRole("tab",{name:/发送报文/}).getAttribute("aria-selected")).toBe("true");
});
it("a half-close confirmation cannot send into a changed request",()=>{
  const view=render(<TcpWorkbench/>);fireEvent.click(screen.getByRole("button",{name:"半关闭"}));
  const guard=vi.mocked(state.setGuard).mock.calls[0][0]!;state={...state,request:{...state.request!,id:"changed"}};
  view.rerender(<TcpWorkbench/>);if(!guard||typeof guard==="function")throw new Error("Expected concrete guard");guard.action();expect(state.protocolSession.send).not.toHaveBeenCalled();
});
it("half-closed sessions reject another send in the UI",()=>{
  state={...state,protocolSession:{...state.protocolSession,session:{...state.protocolSession.session!,client_half_closed:true}}};
  render(<TcpWorkbench/>);expect((screen.getByRole("button",{name:"发送报文"}) as HTMLButtonElement).disabled).toBe(true);expect((screen.getByRole("button",{name:"半关闭"}) as HTMLButtonElement).disabled).toBe(true);
});
