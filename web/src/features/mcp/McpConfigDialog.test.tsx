// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { api, saveFile } from "../../shared/api";
import { useWorkbench } from "../workbench/context";
import McpConfigDialog from "./McpConfigDialog";
import { mcpConfig } from "./model";
vi.mock("../workbench/context",()=>({useWorkbench:vi.fn()}));
vi.mock("../../shared/api",()=>({api:vi.fn(),saveFile:vi.fn()}));
vi.mock("../../shared/ui",()=>({
  Editor:({value,onChange,label}:{value:string;onChange:(value:string)=>void;label:string})=><textarea aria-label={label} value={value} onChange={event=>onChange(event.target.value)}/>,
  Choice:({value,label,options,onChange}:{value:string;label:string;options:{value:string;label:string}[];onChange:(value:string)=>void})=><select value={value} aria-label={label} onChange={event=>onChange(event.target.value)}>{options.map(value=><option key={value.value} value={value.value}>{value.label}</option>)}</select>,
  Field:({label,children}:{label:string;children:React.ReactNode})=><label>{label}{children}</label>,
}));
afterEach(cleanup);
beforeEach(()=>{vi.mocked(api).mockReset();vi.mocked(saveFile).mockReset();});
function setup(){
  const state={authenticated:true,accountId:"first",draft:{id:"workspace",data:{active_environment_id:"dev"}},request:{id:"request",name:"MCP",protocol:mcpConfig()},dark:false,dirty:false,save:vi.fn().mockResolvedValue(true),setGuard:vi.fn(),updateRequest:vi.fn()};
  vi.mocked(useWorkbench).mockImplementation(()=>state as unknown as ReturnType<typeof useWorkbench>);return state;
}
it("an old confirmation cannot apply after closing and reopening the source dialog",()=>{
  const state=setup(),onOpenChange=vi.fn();
  const view=render(<McpConfigDialog open onOpenChange={onOpenChange}/>);
  fireEvent.change(screen.getByLabelText("MCP Host JSON"),{target:{value:'{"url":"https://example.test/mcp"}'}});
  fireEvent.click(screen.getByRole("button",{name:"验证配置"}));
  fireEvent.click(screen.getByRole("button",{name:"使用所选配置"}));
  const action=state.setGuard.mock.calls[0][0].action;
  view.rerender(<McpConfigDialog open={false} onOpenChange={onOpenChange}/>);
  view.rerender(<McpConfigDialog open onOpenChange={onOpenChange}/>);
  act(()=>action());
  expect(state.updateRequest).not.toHaveBeenCalled();expect(api).not.toHaveBeenCalled();
});
it("default host export uses centralized redaction and discards a late owner's result",async()=>{
  const state=setup();let resolve!:(value:unknown)=>void;
  vi.mocked(api).mockImplementation(()=>new Promise(done=>{resolve=done;}));
  const view=render(<McpConfigDialog open onOpenChange={vi.fn()}/>);
  fireEvent.click(screen.getByRole("button",{name:"导出 Host 配置"}));
  expect(api).toHaveBeenCalledWith("/api/workspaces/workspace/export","POST",{format:"moleapi",include_secrets:false});
  state.accountId="second";view.rerender(<McpConfigDialog open onOpenChange={vi.fn()}/>);
  await act(async()=>{resolve({content:"{}"});});
  expect(saveFile).not.toHaveBeenCalled();
});
it("switches retained local JSON validation copy while keeping the invalid configuration draft", async () => {
  const { setLanguage } = await import("../../shared/i18n");
  setup();
  const draft = '{"mcpServers":{"我的服务":{"args":"invalid"}}}';
  render(<McpConfigDialog open onOpenChange={vi.fn()} />);
  fireEvent.change(screen.getByLabelText("MCP Host JSON"), { target: { value: draft } });
  fireEvent.click(screen.getByRole("button", { name: "验证配置" }));
  const original = screen.getByRole("alert").textContent;
  expect(original).toContain("我的服务");
  await act(async () => { await setLanguage("en"); });
  expect(screen.getByRole("alert").textContent).toContain("我的服务");
  expect(screen.getByRole("alert").textContent).not.toBe(original);
  expect((screen.getByLabelText("MCP Host JSON") as HTMLTextAreaElement).value).toBe(draft);
  expect(api).not.toHaveBeenCalled();
});
