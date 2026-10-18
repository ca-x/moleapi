// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import {newRequest} from "../../shared/model";
import type {WorkspaceData} from "../../shared/types";
import ApiReference from "./ApiReference";

afterEach(()=>{cleanup();vi.unstubAllGlobals();});

function fixture():WorkspaceData {
  const first=newRequest("First","https://example.test/items?token=owner-only");
  first.id="first";
  first.method="POST";
  first.description="# Guide\n\n## Details\n\n<script>alert(1)</script> ![Remote](https://evil.test/pixel.png)";
  first.query=[{id:"q1",key:"page",value:"",enabled:true},{id:"q2",key:"hidden-query",value:"x",enabled:false},{id:"q3",key:" ",value:"space-key",enabled:true}];
  first.headers=[{id:"h1",key:"X-Trace",value:"owner-only",enabled:true},{id:"h2",key:"hidden-header",value:"x",enabled:false}];
  first.body_kind="json";
  first.body='<img src="https://evil.test/body.png">';
  first.examples=[{id:"e1",name:"Created",status:201,headers:[{id:"eh",key:"Content-Type",value:"application/json",enabled:true}],body:'<script>alert("example")</script>'}];
  const second=newRequest("Second","https://example.test/second");
  second.id="second";
  second.description="## Starts at two";
  const other=newRequest("Subscription","wss://example.test");
  other.id="other";
  other.protocol={kind:"websocket"};
  return {schema_version:1,collections:[{id:"c1",name:"First group",description:"",requests:[first,other]},{id:"c2",name:"Second group",description:"",requests:[second]}],environments:[],active_environment_id:null};
}

it("renders ordered HTTP requests with exact inert source and enabled fields only",async()=>{
  await setLanguage("en");
  const page=render(<Theme><ApiReference data={fixture()} scopeKey="account/workspace"/></Theme>);
  expect(screen.getByRole("navigation",{name:"API reference requests"}).textContent).toContain("First group");
  expect(screen.getByRole("navigation",{name:"API reference requests"}).textContent).toContain("Second group");
  expect(screen.getByText("1 non-HTTP request omitted from this reference.")).toBeTruthy();
  expect(screen.getByRole("heading",{name:"First"})).toBeTruthy();
  expect(screen.getByRole("heading",{name:/Guide/,level:4})).toBeTruthy();
  expect(screen.getByRole("heading",{name:"Details",level:5})).toBeTruthy();
  expect(screen.getByText("https://example.test/items?token=owner-only")).toBeTruthy();
  expect(screen.getByText("page")).toBeTruthy();
  const pageRow=screen.getByText("page").closest("tr");
  expect(pageRow?.querySelector("td")?.textContent).toBe("");
  const spaceRow=Array.from(page.container.querySelectorAll(".api-reference-table-scroll tbody tr")).find(row=>row.querySelector("th")?.textContent===" ");
  expect(spaceRow?.querySelector("td")?.textContent).toBe("space-key");
  expect(screen.getByText("X-Trace")).toBeTruthy();
  expect(screen.queryByText("hidden-query")).toBeNull();
  expect(screen.queryByText("hidden-header")).toBeNull();
  expect(screen.getByText('<img src="https://evil.test/body.png">')).toBeTruthy();
  expect(screen.getByText('<script>alert("example")</script>')).toBeTruthy();
  expect(screen.getByRole("heading",{name:"Headers",level:5})).toBeTruthy();
  expect(page.container.querySelector("script,img")).toBeNull();
  expect(screen.queryByRole("button",{name:"Subscription"})).toBeNull();
});

it("follows the current draft and resets selection when the workspace changes",async()=>{
  await setLanguage("en");
  const data=fixture();
  const page=render(<Theme><ApiReference data={data} scopeKey="owner/first"/></Theme>);
  fireEvent.click(screen.getByRole("button",{name:"GET Second"}));
  expect(screen.getByRole("heading",{name:"Second"})).toBeTruthy();
  expect(screen.getByRole("heading",{name:"Starts at two",level:4})).toBeTruthy();
  const changed={...data,collections:data.collections.map(collection=>({...collection,requests:collection.requests.filter(request=>request.id!=="second")}))};
  page.rerender(<Theme><ApiReference data={changed} scopeKey="owner/first"/></Theme>);
  expect(screen.getByRole("heading",{name:"First"})).toBeTruthy();
  page.rerender(<Theme><ApiReference data={data} scopeKey="other/second"/></Theme>);
  expect(screen.getByRole("heading",{name:"First"})).toBeTruthy();
});

it("handles a workspace with no HTTP requests",async()=>{
  await setLanguage("en");
  const data=fixture();
  data.collections[0].requests=data.collections[0].requests.filter(request=>request.protocol?.kind==="websocket");
  data.collections[1].requests=[];
  render(<Theme><ApiReference data={data} scopeKey="owner/empty"/></Theme>);
  expect(screen.getByText("No HTTP requests to document yet.")).toBeTruthy();
});

it("selects the next HTTP request after a selected request is deleted or changes protocol",async()=>{
  await setLanguage("en");
  const data=fixture();
  const third=newRequest("Third","https://example.test/third");
  third.id="third";
  data.collections[1].requests.push(third);
  const page=render(<Theme><ApiReference data={data} scopeKey="owner/workspace"/></Theme>);
  fireEvent.click(screen.getByRole("button",{name:"GET Second"}));
  const deleted={...data,collections:data.collections.map(collection=>({...collection,requests:collection.requests.filter(request=>request.id!=="second")}))};
  page.rerender(<Theme><ApiReference data={deleted} scopeKey="owner/workspace"/></Theme>);
  expect(screen.getByRole("heading",{name:"Third"})).toBeTruthy();
  page.rerender(<Theme><ApiReference data={data} scopeKey="owner/other"/></Theme>);
  fireEvent.click(screen.getByRole("button",{name:"GET Second"}));
  const changed={...data,collections:data.collections.map(collection=>({...collection,requests:collection.requests.map(request=>request.id==="second"?{...request,protocol:{kind:"sse" as const}}:request)}))};
  page.rerender(<Theme><ApiReference data={changed} scopeKey="owner/other"/></Theme>);
  expect(screen.getByRole("heading",{name:"Third"})).toBeTruthy();
});

it("copies only the selected request URI without changing the draft",async()=>{
  await setLanguage("en");
  const writeText=vi.fn().mockResolvedValue(undefined);
  vi.stubGlobal("navigator",Object.assign(Object.create(navigator),{clipboard:{writeText}}));
  const data=fixture();
  const original=JSON.stringify(data);
  render(<Theme><ApiReference data={data} scopeKey="owner/workspace"/></Theme>);
  fireEvent.click(screen.getByRole("button",{name:"Copy request link"}));
  await waitFor(()=>expect(writeText).toHaveBeenCalledExactlyOnceWith("moleapi://request/first"));
  expect(screen.getByRole("status").textContent).toBe("Request link copied.");
  expect(JSON.stringify(data)).toBe(original);
});

it("reports clipboard failure without navigating or changing the draft",async()=>{
  await setLanguage("en");
  const writeText=vi.fn().mockRejectedValue(new Error("denied"));
  vi.stubGlobal("navigator",Object.assign(Object.create(navigator),{clipboard:{writeText}}));
  render(<Theme><ApiReference data={fixture()} scopeKey="owner/workspace"/></Theme>);
  fireEvent.click(screen.getByRole("button",{name:"Copy request link"}));
  await waitFor(()=>expect(screen.getByRole("alert").textContent).toBe("Could not copy the request link."));
});
