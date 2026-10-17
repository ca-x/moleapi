// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import {newRequest} from "../../shared/model";
import ExamplesEditor from "./ExamplesEditor";

const previewMock=vi.hoisted(()=>vi.fn());
vi.mock("../../shared/api",()=>({previewMock,token:()=>null}));

vi.mock("../../shared/ui",()=>({
  Editor:({label,value,onChange}:{label?:string;value:string;onChange?:(value:string)=>void})=><textarea aria-label={label??"example-body"} value={value} readOnly={!onChange} onChange={event=>onChange?.(event.target.value)}/>,
  Field:({label,children}:{label:string;children:React.ReactNode})=><label>{label}{children}</label>,
  PairEditor:({rows}:{rows:unknown[]})=><div data-testid="match-pairs">{rows.length}</div>,
  ToolButton:({label,onClick,children}:{label:string;onClick:()=>void;children:React.ReactNode})=><button aria-label={label} onClick={onClick}>{children}</button>
}));
afterEach(cleanup);

it("edits an opt-in mock implementation without replacing the saved example",async()=>{
  await setLanguage("en");
  const request=newRequest("R","https://example.test/");
  request.examples=[{id:"e",name:"Example",status:200,headers:[],body:"static"}];
  const update=vi.fn();
  previewMock.mockResolvedValue({status:202,body:"dynamic"});
  const page=render(<Theme><ExamplesEditor request={request} update={update} dark={false} asyncMock workspaceId="w"/></Theme>);
  expect(screen.getByText(/api\/mock\/w/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"Preview Mock"}));
  expect((await screen.findByRole("textbox",{name:/Mock preview result/}) as HTMLTextAreaElement).value).toBe("dynamic");
  fireEvent.change(screen.getByRole("textbox",{name:"Mock implementation script"}),{target:{value:"pm.mock.setBody('dynamic')"}});
  expect(update.mock.calls[0][0].examples[0]).toEqual({...request.examples[0],mock_script:"pm.mock.setBody('dynamic')"});
  page.rerender(<Theme><ExamplesEditor request={request} update={update} dark={false} asyncMock={false} workspaceId="w"/></Theme>);
  expect(screen.queryByRole("textbox",{name:"Mock implementation script"})).toBeNull();
  expect(screen.getByText("Enable v2 script mode in workspace settings to edit dynamic Mocks.")).toBeTruthy();
  page.rerender(<Theme><ExamplesEditor request={request} update={update} dark={false} asyncMock workspaceId="w" dirty/></Theme>);
  expect(screen.getByRole("button",{name:"Preview Mock"})).toHaveProperty("disabled",true);
});

it("shows the mock editor in Chinese",async()=>{
  await setLanguage("zh-CN");
  const request=newRequest("R","https://example.test/");
  request.examples=[{id:"e",name:"Example",status:200,headers:[],body:"static"}];
  render(<Theme><ExamplesEditor request={request} update={vi.fn()} dark={false} asyncMock workspaceId="w"/></Theme>);
  expect(screen.getByRole("textbox",{name:"Mock 实现脚本"})).toBeTruthy();
});

it("edits a saved example's Mock matching rule and shows its routed URL",async()=>{
  await setLanguage("en");
  const request=newRequest("R","https://example.test/items");
  request.examples=[{id:"e",name:"Example",status:200,headers:[],body:"static"}];
  const update=vi.fn();
  const page=render(<Theme><ExamplesEditor request={request} update={update} dark={false} workspaceId="w"/></Theme>);
  expect(screen.getByText(/POST|GET \/api\/mocks\/w\/items/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button",{name:"Add Mock match conditions"}));
  expect(update.mock.calls[0][0].examples[0].mock_match).toEqual({priority:0,query:[],headers:[],body:""});
  request.examples=[{...request.examples[0],mock_match:{priority:0,query:[],headers:[],body:""}}];
  page.rerender(<Theme><ExamplesEditor request={request} update={update} dark={false} workspaceId="w"/></Theme>);
  fireEvent.change(screen.getByRole("spinbutton",{name:"Match priority"}),{target:{value:"9"}});
  expect(update.mock.lastCall?.[0].examples[0].mock_match.priority).toBe(9);
  fireEvent.click(screen.getByRole("button",{name:"Remove match conditions"}));
  expect(update.mock.lastCall?.[0].examples[0].mock_match).toBeUndefined();
});

it("loads saved match rules when empty condition arrays were omitted from JSON",async()=>{
  await setLanguage("en");
  const request=newRequest("R","https://example.test/items");
  request.examples=[{id:"e",name:"Example",status:202,headers:[],body:"matched",mock_match:{priority:4,query:[{id:"q",key:"mode",value:"fast",enabled:true}]}}];
  render(<Theme><ExamplesEditor request={request} update={vi.fn()} dark={false} workspaceId="w"/></Theme>);
  expect(screen.getByRole("spinbutton",{name:"Match priority"})).toHaveProperty("value","4");
  expect(screen.getAllByTestId("match-pairs")).toHaveLength(2);
});
