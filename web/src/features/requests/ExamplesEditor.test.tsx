// @vitest-environment jsdom
import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import {newRequest} from "../../shared/model";
import ExamplesEditor from "./ExamplesEditor";

const previewMock=vi.hoisted(()=>vi.fn());
const api=vi.hoisted(()=>vi.fn());
vi.mock("../../shared/api",()=>({api,native:false,previewMock,token:()=>null}));
vi.mock("./MockCallActivity",()=>({default:()=>null}));
vi.mock("./MockPublicShares",()=>({default:()=>null}));

vi.mock("../../shared/ui",()=>({
  Editor:({label,value,onChange}:{label?:string;value:string;onChange?:(value:string)=>void})=><textarea aria-label={label??"example-body"} value={value} readOnly={!onChange} onChange={event=>onChange?.(event.target.value)}/>,
  Field:({label,children}:{label:string;children:React.ReactNode})=><label>{label}{children}</label>,
  PairEditor:({rows}:{rows:unknown[]})=><div data-testid="match-pairs">{rows.length}</div>,
  ToolButton:({label,onClick,children}:{label:string;onClick:()=>void;children:React.ReactNode})=><button aria-label={label} onClick={onClick}>{children}</button>
}));
afterEach(()=>{cleanup();api.mockReset();vi.unstubAllGlobals();Reflect.deleteProperty(HTMLElement.prototype,"scrollIntoView");});

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

it("shows name inference only for a selected Schema and clears it with the source",async()=>{
  await setLanguage("en");
  const request=newRequest("R","https://example.test/items");
  request.examples=[{id:"e",name:"Example",status:200,headers:[],body:"static"}];
  const update=vi.fn();
  const page=render(<Theme><ExamplesEditor request={request} update={update} dark={false} workspaceId="w"/></Theme>);
  expect(screen.queryByRole("checkbox",{name:"Use field names for Mock values"})).toBeNull();
  request.examples=[{...request.examples[0],mock_schema:'{"type":"object","properties":{"name":{"type":"string"}},"required":["name"]}'}];
  page.rerender(<Theme><ExamplesEditor request={request} update={update} dark={false} workspaceId="w"/></Theme>);
  fireEvent.click(screen.getByRole("checkbox",{name:"Use field names for Mock values"}));
  expect(update.mock.lastCall?.[0].examples[0]).toMatchObject({mock_name_inference:true,body:"static"});
  request.examples=[update.mock.lastCall?.[0].examples[0]];
  page.rerender(<Theme><ExamplesEditor request={request} update={update} dark={false} workspaceId="w"/></Theme>);
  fireEvent.click(screen.getByRole("checkbox",{name:"Generate Mock body from JSON Schema"}));
  expect(update.mock.lastCall?.[0].examples[0]).toMatchObject({body:"static"});
  expect(update.mock.lastCall?.[0].examples[0].mock_name_inference).toBeFalsy();
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
  api.mockResolvedValue({method:"GET",status:200,media_type:"application/json"});
  render(<Theme><ExamplesEditor request={request} update={update} dark={false} workspaceId="w" workspaceRevision={1} specifications={specifications}/></Theme>);
  fireEvent.click(screen.getByRole("combobox",{name:"Mock body source"}));
  fireEvent.click(screen.getByRole("option",{name:/GET \/items · 200 application\/json/}));
  await waitFor(()=>expect(update).toHaveBeenCalled());
  expect(api).toHaveBeenCalledWith("/api/workspaces/w/mock-schema/validate","POST",{request_id:request.id,specification_id:"spec",pointer:"#/paths/~1items/get/responses/200/content/application~1json/schema",expected_revision:1});
  expect(update.mock.lastCall?.[0].examples[0]).toMatchObject({status:200,headers:[{value:"application/json"}],body:"static",mock_schema_link:{specification_id:"spec",pointer:"#/paths/~1items/get/responses/200/content/application~1json/schema"}});
  expect(JSON.stringify(update.mock.lastCall?.[0].examples[0])).not.toContain("mock_schema\":\"\"");
});

it("rejects unsupported linked choices before changing the saved example",async()=>{
  vi.stubGlobal("ResizeObserver",class {observe(){} unobserve(){} disconnect(){}});
  HTMLElement.prototype.scrollIntoView=()=>{};
  await setLanguage("en");
  const request=newRequest("R","https://example.test/items");
  request.specification_id="spec";
  request.examples=[{id:"e",name:"Example",status:200,headers:[],body:"static"}];
  const source=JSON.stringify({openapi:"3.1.0",paths:{"/items":{get:{responses:{"200":{content:{"application/json":{schema:{type:"string",pattern:".*"}}}}}}}}});
  const update=vi.fn();
  api.mockRejectedValueOnce(new Error("Unsupported linked Mock schema keyword pattern"));
  render(<Theme><ExamplesEditor request={request} update={update} dark={false} workspaceId="w" workspaceRevision={1} specifications={[{id:"spec",name:"Spec",kind:"openapi",source,dialect:"3.1.0"}]}/></Theme>);
  fireEvent.click(screen.getByRole("combobox",{name:"Mock body source"}));
  fireEvent.click(screen.getByRole("option",{name:/GET \/items · 200 application\/json/}));
  expect((await screen.findByRole("alert")).textContent).toContain("Unsupported linked Mock schema keyword pattern");
  expect(update).not.toHaveBeenCalled();
});

it("shows a stale link when the request switches to a different specification with the same pointer",async()=>{
  vi.stubGlobal("ResizeObserver",class {observe(){} unobserve(){} disconnect(){}});
  HTMLElement.prototype.scrollIntoView=()=>{};
  await setLanguage("en");
  const request=newRequest("R","https://example.test/items");
  request.specification_id="new";
  const pointer="#/paths/~1items/get/responses/200/content/application~1json/schema";
  request.examples=[{id:"e",name:"Example",status:200,headers:[],body:"static",mock_schema_link:{specification_id:"old",pointer}}];
  const source=JSON.stringify({openapi:"3.1.0",paths:{"/items":{get:{responses:{"200":{content:{"application/json":{schema:{type:"string"}}}}}}}}});
  const update=vi.fn();
  render(<Theme><ExamplesEditor request={request} update={update} dark={false} workspaceId="w" specifications={[{id:"new",name:"New",kind:"openapi",source,dialect:"3.1.0"}]}/></Theme>);
  expect(screen.getByRole("combobox",{name:"Mock body source"}).textContent).toContain("Linked response unavailable");
  expect(screen.getByRole("alert").textContent).toContain("does not match this specification");
  fireEvent.click(screen.getByRole("button",{name:"Clear schema link"}));
  expect(update.mock.lastCall?.[0].examples[0].mock_schema_link).toBeUndefined();
});

it("keeps newer example edits while linked-response validation is pending",async()=>{
  vi.stubGlobal("ResizeObserver",class {observe(){} unobserve(){} disconnect(){}});
  HTMLElement.prototype.scrollIntoView=()=>{};
  await setLanguage("en");
  const request=newRequest("R","https://example.test/items");
  request.specification_id="spec";
  request.examples=[{id:"e",name:"Before",status:200,headers:[{id:"h",key:"Content-Type",value:"text/plain",enabled:true}],body:"static"}];
  const source=JSON.stringify({openapi:"3.1.0",paths:{"/items":{get:{responses:{"200":{content:{"application/json":{schema:{type:"string"}}}}}}}}});
  const specifications=[{id:"spec",name:"Spec",kind:"openapi",source,dialect:"3.1.0"}];
  let resolveValidation!:(value:{method:string;status:number;media_type:string})=>void;
  api.mockImplementation(()=>new Promise(resolve=>{resolveValidation=resolve;}));
  const update=vi.fn();
  const page=render(<Theme><ExamplesEditor request={request} update={update} dark={false} workspaceId="w" workspaceRevision={1} specifications={specifications}/></Theme>);
  fireEvent.click(screen.getByRole("combobox",{name:"Mock body source"}));
  fireEvent.click(screen.getByRole("option",{name:/GET \/items · 200 application\/json/}));
  const updatedRequest={...request,examples:[{...request.examples[0],name:"After",headers:[{...request.examples[0].headers[0],value:"text/html"}]}]};
  page.rerender(<Theme><ExamplesEditor request={updatedRequest} update={update} dark={false} workspaceId="w" workspaceRevision={1} specifications={specifications}/></Theme>);
  await act(async()=>resolveValidation({method:"GET",status:200,media_type:"application/json"}));
  await waitFor(()=>expect(update).toHaveBeenCalled());
  expect(update.mock.lastCall?.[0].examples[0]).toMatchObject({name:"After",headers:[{value:"application/json"}]});
  let resolveSecond!:(value:{method:string;status:number;media_type:string})=>void;
  api.mockImplementation(()=>new Promise(resolve=>{resolveSecond=resolve;}));
  fireEvent.click(screen.getByRole("combobox",{name:"Mock body source"}));
  fireEvent.click(screen.getByRole("option",{name:/GET \/items · 200 application\/json/}));
  page.rerender(<Theme><ExamplesEditor request={{...updatedRequest,method:"POST"}} update={update} dark={false} workspaceId="w" workspaceRevision={1} specifications={specifications}/></Theme>);
  await act(async()=>resolveSecond({method:"GET",status:200,media_type:"application/json"}));
  expect(update).toHaveBeenCalledTimes(1);
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

it("edits and reloads bounded Mock fault controls in both languages",async()=>{
  await setLanguage("en");
  const request=newRequest("R","https://example.test/items");
  request.examples=[{id:"e",name:"Example",status:200,headers:[],body:"static"}];
  const update=vi.fn();
  const page=render(<Theme><ExamplesEditor request={request} update={update} dark={false} workspaceId="w"/></Theme>);
  expect(screen.getByRole("spinbutton",{name:"Failure status (5xx)"})).toHaveProperty("value","503");
  fireEvent.change(screen.getByRole("spinbutton",{name:"Failure rate (%)"}),{target:{value:"120"}});
  expect(update.mock.lastCall?.[0].examples[0].fault_rate_percent).toBe(100);
  request.examples=[{...request.examples[0],fault_rate_percent:100,fault_status:503}];
  page.rerender(<Theme><ExamplesEditor request={request} update={update} dark={false} workspaceId="w"/></Theme>);
  fireEvent.change(screen.getByRole("spinbutton",{name:"Failure status (5xx)"}),{target:{value:"502"}});
  expect(update.mock.lastCall?.[0].examples[0].fault_status).toBe(502);
  request.examples=[{...request.examples[0],fault_rate_percent:0,fault_status:502}];
  page.rerender(<Theme><ExamplesEditor request={request} update={update} dark={false} workspaceId="w"/></Theme>);
  expect(screen.getByRole("spinbutton",{name:"Failure status (5xx)"})).toHaveProperty("value","502");
  await setLanguage("zh-CN");
  expect(screen.getByRole("spinbutton",{name:"故障比例（%）"})).toBeTruthy();
  expect(screen.getByRole("spinbutton",{name:"故障状态码（5xx）"})).toBeTruthy();
});

it("edits and reloads the per-minute Mock limit in both languages",async()=>{
  await setLanguage("en");
  const request=newRequest("R","https://example.test/items");
  request.examples=[{id:"e",name:"Example",status:200,headers:[],body:"static"}];
  const update=vi.fn();
  const page=render(<Theme><ExamplesEditor request={request} update={update} dark={false} workspaceId="w"/></Theme>);
  const control=()=>screen.getByRole("spinbutton",{name:"Request limit per minute"});
  expect(control()).toHaveProperty("value","0");
  fireEvent.change(control(),{target:{value:"1200"}});
  expect(update.mock.lastCall?.[0].examples[0].rate_limit_per_minute).toBe(1000);
  request.examples=[{...request.examples[0],rate_limit_per_minute:2}];
  page.rerender(<Theme><ExamplesEditor request={request} update={update} dark={false} workspaceId="w"/></Theme>);
  expect(control()).toHaveProperty("value","2");
  await setLanguage("zh-CN");
  expect(screen.getByRole("spinbutton",{name:"每分钟请求上限"})).toBeTruthy();
});
