// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import {newRequest} from "../../shared/model";
import ExamplesEditor from "./ExamplesEditor";

const previewMock=vi.hoisted(()=>vi.fn());
vi.mock("../../shared/api",()=>({native:false,previewMock,token:()=>null}));
vi.mock("./MockCallActivity",()=>({default:()=>null}));
vi.mock("./MockPublicShares",()=>({default:()=>null}));

vi.mock("../../shared/ui",()=>({
  Editor:({label,value,onChange}:{label?:string;value:string;onChange?:(value:string)=>void})=><textarea aria-label={label??"example-body"} value={value} readOnly={!onChange} onChange={event=>onChange?.(event.target.value)}/>,
  Field:({label,children}:{label:string;children:React.ReactNode})=><label>{label}{children}</label>,
  PairEditor:({rows}:{rows:unknown[]})=><div data-testid="match-pairs">{rows.length}</div>,
  ToolButton:({label,onClick,children}:{label:string;onClick:()=>void;children:React.ReactNode})=><button aria-label={label} onClick={onClick}>{children}</button>
}));
afterEach(()=>{cleanup();vi.unstubAllGlobals();Reflect.deleteProperty(HTMLElement.prototype,"scrollIntoView");});

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

it("keeps the static body while editing an explicit Mock JSON Schema",async()=>{
  await setLanguage("en");
  const request=newRequest("R","https://example.test/items");
  request.examples=[{id:"e",name:"Example",status:200,headers:[],body:"static"}];
  const update=vi.fn();
  const page=render(<Theme><ExamplesEditor request={request} update={update} dark={false} workspaceId="w"/></Theme>);
  fireEvent.click(screen.getByRole("checkbox",{name:"Generate Mock body from JSON Schema"}));
  expect(update.mock.lastCall?.[0].examples[0].body).toBe("static");
  request.examples=[{...request.examples[0],mock_schema:update.mock.lastCall?.[0].examples[0].mock_schema}];
  page.rerender(<Theme><ExamplesEditor request={request} update={update} dark={false} workspaceId="w"/></Theme>);
  fireEvent.click(screen.getByRole("button",{name:"Edit Mock JSON Schema"}));
  fireEvent.change(screen.getByRole("textbox",{name:"Mock JSON Schema"}),{target:{value:""}});
  expect(screen.getByRole("textbox",{name:"Mock JSON Schema"})).toBeTruthy();
  expect(screen.getByRole("alert").textContent).toContain("Schema must be a JSON object");
  expect(screen.getByRole("button",{name:"Done"})).toHaveProperty("disabled",true);
  const revised='{"type":"object","properties":{"id":{"const":3}},"required":["id"]}';
  fireEvent.change(screen.getByRole("textbox",{name:"Mock JSON Schema"}),{target:{value:revised}});
  fireEvent.click(screen.getByRole("button",{name:"Done"}));
  expect(update.mock.lastCall?.[0].examples[0].mock_schema).toBe(revised);
  expect(update.mock.lastCall?.[0].examples[0].body).toBe("static");
  await setLanguage("zh-CN");
  expect(screen.getByRole("button",{name:"编辑 Mock JSON Schema"})).toBeTruthy();
});

it("selects a saved OpenAPI response without an operationId and keeps the static body",async()=>{
  vi.stubGlobal("ResizeObserver",class {observe(){} unobserve(){} disconnect(){}});
  HTMLElement.prototype.scrollIntoView=()=>{};
  await setLanguage("en");
  const request=newRequest("R","https://example.test/items");
  request.specification_id="spec";
  request.examples=[{id:"e",name:"Example",status:201,headers:[{id:"h",key:"Content-Type",value:"text/plain",enabled:true}],body:"static"}];
  const source=JSON.stringify({openapi:"3.1.0",paths:{"/items":{get:{responses:{"200":{content:{"application/json":{schema:{type:"string",example:"linked"}}}}}}}}});
  const specifications=[{id:"spec",name:"Pet API",kind:"openapi",source,dialect:"3.1.0"}];
  const update=vi.fn();
  render(<Theme><ExamplesEditor request={request} update={update} dark={false} workspaceId="w" specifications={specifications}/></Theme>);
  fireEvent.click(screen.getByRole("combobox",{name:"Mock body source"}));
  fireEvent.click(screen.getByRole("option",{name:/GET \/items · 200 application\/json/}));
  expect(update.mock.lastCall?.[0].examples[0]).toMatchObject({status:200,headers:[{value:"application/json"}],body:"static",mock_schema_link:{specification_id:"spec",pointer:"#/paths/~1items/get/responses/200/content/application~1json/schema"}});
  expect(JSON.stringify(update.mock.lastCall?.[0].examples[0])).not.toContain("mock_schema\":\"\"");
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

it("does not advertise nested URL templates as routed Mock paths",async()=>{
  await setLanguage("en");
  const request=newRequest("R","https://example.test/v1/{{id}}");
  request.examples=[{id:"e",name:"Example",status:200,headers:[],body:"static"}];
  render(<Theme><ExamplesEditor request={request} update={vi.fn()} dark={false} workspaceId="w"/></Theme>);
  expect(screen.getByText(/single \{name\} segment/)).toBeTruthy();
  expect(screen.queryByText(/\/api\/mocks\/w\/v1/)).toBeNull();
});

it("edits and reloads a bounded saved Mock delay in both languages",async()=>{
  await setLanguage("en");
  const request=newRequest("R","https://example.test/items");
  request.examples=[{id:"e",name:"Example",status:200,headers:[],body:"static"}];
  const update=vi.fn();
  const page=render(<Theme><ExamplesEditor request={request} update={update} dark={false} workspaceId="w"/></Theme>);
  fireEvent.change(screen.getByRole("spinbutton",{name:"Mock delay (ms)"}),{target:{value:"120"}});
  expect(update.mock.lastCall?.[0].examples[0].delay_ms).toBe(120);
  for(const [typed,expected] of [["-1",0],["30001",30_000],["1.5",2]] as const){
    fireEvent.change(screen.getByRole("spinbutton",{name:"Mock delay (ms)"}),{target:{value:typed}});
    expect(update.mock.lastCall?.[0].examples[0].delay_ms).toBe(expected);
  }
  request.examples=[{...request.examples[0],delay_ms:120}];
  page.rerender(<Theme><ExamplesEditor request={request} update={update} dark={false} workspaceId="w"/></Theme>);
  expect(screen.getByRole("spinbutton",{name:"Mock delay (ms)"})).toHaveProperty("value","120");
  await setLanguage("zh-CN");
  expect(screen.getByRole("spinbutton",{name:"Mock 延迟（毫秒）"})).toBeTruthy();
});
