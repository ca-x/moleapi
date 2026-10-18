// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import {api} from "../../shared/api";
import {useWorkbench} from "../workbench/context";
import DocumentPublications from "./DocumentPublications";

vi.mock("../../shared/api",()=>({api:vi.fn(),native:false}));
vi.mock("../workbench/context",()=>({useWorkbench:vi.fn()}));
afterEach(()=>{cleanup();vi.clearAllMocks();});

function setup(dirty=false) {
  const draft={id:"w",name:"Private workspace",revision:2,data:{documents:[
    {id:"guide",name:"Guide",source:"# Public",secret:false},
    {id:"private",name:"Secret",source:"private-sentinel",secret:true}
  ]}};
  const context={draft,saved:draft,accountId:"owner",dirty,setGuard:vi.fn(),stateRef:{current:{draft,saved:draft}},accountRef:{current:"owner"}};
  vi.mocked(useWorkbench).mockReturnValue(context as never);
  vi.mocked(api).mockImplementation(async(path,method="GET")=>{
    if(method==="GET")return [] as never;
    if(method==="POST")return {id:"a".repeat(32),title:"Site",created_at:1,source_revision:2,document_titles:["Guide"],public_url:`/docs/${"a".repeat(32)}`,revoked:false} as never;
    throw new Error(`Unexpected ${method} ${path}`);
  });
  return context;
}

it("requires a saved workspace and deliberate public document confirmation",async()=>{
  await setLanguage("en");
  const state=setup(true);
  const page=render(<Theme><DocumentPublications/></Theme>);
  await waitFor(()=>expect(screen.getByText("No documents have been published.")).toBeTruthy());
  expect(screen.queryByText("Secret")).toBeNull();
  fireEvent.change(screen.getByRole("textbox",{name:"Public site title"}),{target:{value:"Site"}});
  fireEvent.click(screen.getByRole("checkbox"));
  expect(screen.getByRole("button",{name:"Publish selected documents"}).hasAttribute("disabled")).toBe(true);
  state.dirty=false;
  vi.mocked(useWorkbench).mockReturnValue({...state,dirty:false} as never);
  page.rerender(<Theme><DocumentPublications/></Theme>);
  fireEvent.click(screen.getByRole("button",{name:"Publish selected documents"}));
  expect(state.setGuard).toHaveBeenCalledOnce();
  expect(vi.mocked(api).mock.calls.filter(([,method])=>method==="POST")).toHaveLength(0);
  state.setGuard.mock.lastCall?.[0].action();
  await waitFor(()=>expect(vi.mocked(api).mock.calls.filter(([,method])=>method==="POST")).toHaveLength(1));
  const body=vi.mocked(api).mock.calls.find(([,method])=>method==="POST")?.[2] as {document_ids:string[];expected_revision:number};
  expect(body.document_ids).toEqual(["guide"]);
  expect(body.expected_revision).toBe(2);
  expect((await screen.findByRole("link",{name:"Open public page"})).getAttribute("href")).toBe(`/docs/${"a".repeat(32)}`);
});
