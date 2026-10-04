// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { useWorkbench } from "../workbench/context";
import McpWorkbench from "./McpWorkbench";
import { mcpConfig } from "./model";
vi.mock("../workbench/context", () => ({ useWorkbench: vi.fn() }));
vi.mock("./McpConfigDialog", () => ({ default: () => null }));
vi.mock("./McpContent", () => ({ default: () => null }));
vi.mock("../../shared/ui", () => ({
  Editor: ({ value, onChange, label, readOnly }: {value:string; onChange?:(value:string)=>void;label:string;readOnly?:boolean}) => <textarea aria-label={label} value={value} readOnly={readOnly} onChange={event=>onChange?.(event.target.value)} />,
  Choice: ({value,label,options,onChange,disabled}:{value:string;label:string;options:{value:string;label:string}[];onChange:(value:string)=>void;disabled?:boolean}) => <select value={value} aria-label={label} disabled={disabled} onChange={event=>onChange(event.target.value)}>{options.map(value=><option key={value.value} value={value.value}>{value.label}</option>)}</select>,
  Field: ({label,children}:{label:string;children:React.ReactNode}) => <label>{label}{children}</label>,
  PairEditor: () => null,
  ToolButton: () => null,
}));
vi.mock("../protocols/SessionEventPane", () => ({ default: ({onReply}:{onReply:(event:unknown)=>void}) => <button onClick={()=>onReply({cursor:1,direction:"incoming",message:{kind:"mcp_callback",callback_id:"callback",method:"sampling/createMessage",params:{}}})}>检查回调</button> }));
afterEach(cleanup);
it("a late callback reply cannot close or mark a new owner's callback answered", async () => {
  let resolve!: (value:boolean)=>void;
  const state = { authenticated:true,accountId:"first",draft:{id:"workspace",data:{active_environment_id:"dev"}},request:{id:"request",protocol:mcpConfig()},dark:false,
    updateRequest:vi.fn(), mcpRun:{current:null}, protocolSession:{session:{id:"session",state:"open"},events:[],send:vi.fn(()=>new Promise<boolean>(done=>{resolve=done;})),close:vi.fn(),busy:false,sending:false,error:"",dropped:0} };
  vi.mocked(useWorkbench).mockImplementation(()=>state as unknown as ReturnType<typeof useWorkbench>);
  const view=render(<McpWorkbench />);
  fireEvent.mouseDown(screen.getByRole("tab",{name:/消息与回调/}),{button:0});
  fireEvent.click(screen.getByRole("button",{name:"检查回调"}));
  fireEvent.click(screen.getByRole("button",{name:"发送响应"}));
  state.accountId="second"; state.protocolSession.session.id="other-session";
  view.rerender(<McpWorkbench />);
  fireEvent.mouseDown(screen.getByRole("tab",{name:/消息与回调/}),{button:0});
  fireEvent.click(screen.getByRole("button",{name:"检查回调"}));
  await act(async()=>{resolve(true);});
  expect(screen.getByRole("dialog",{name:"MCP 客户端回调"})).toBeTruthy();
  expect(state.protocolSession.send).toHaveBeenCalledTimes(1);
});
it("the keyboard dispatcher runs a selected tool on the current connection",async()=>{
  const config={...mcpConfig(),name:"echo",arguments_source:'{"text":"hello"}'};
  const dispatch:{current:(()=>void)|null}={current:null};
  const state={authenticated:true,accountId:"account",draft:{id:"workspace",data:{active_environment_id:"dev"}},request:{id:"request",protocol:config},dark:false,mcpRun:dispatch,updateRequest:vi.fn(),protocolSession:{session:{id:"session",state:"open"},events:[],send:vi.fn().mockResolvedValue(true),connect:vi.fn(),close:vi.fn(),busy:false,sending:false,error:"",dropped:0}};
  vi.mocked(useWorkbench).mockReturnValue(state as unknown as ReturnType<typeof useWorkbench>);
  render(<McpWorkbench/>);
  await act(async()=>{dispatch.current?.();});
  expect(state.protocolSession.send).toHaveBeenCalledWith(expect.objectContaining({kind:"mcp_request",method:"tools/call",name:"echo",arguments_source:'{"text":"hello"}'}));
  expect(state.protocolSession.connect).not.toHaveBeenCalled();
});
