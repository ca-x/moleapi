// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import MockCallActivity from "./MockCallActivity";

const api=vi.hoisted(()=>vi.fn());
const auth=vi.hoisted(()=>({token:"owner"}));
vi.mock("../../shared/api",()=>({api,token:()=>auth.token}));
afterEach(()=>{cleanup();api.mockReset();auth.token="owner";});

it("shows only current-request and unmatched Mock metadata and confirms clear",async()=>{
  await setLanguage("en");
  let calls=[
    {id:"one",request_id:"r",example_id:"e",method:"GET",response_status:202,outcome:"served",at:"2026-10-17T00:00:00Z",elapsed_ms:4},
    {id:"other",request_id:"other",example_id:"f",method:"GET",response_status:200,outcome:"served",at:"2026-10-17T00:00:00Z",elapsed_ms:2},
    {id:"missing",request_id:null,example_id:null,method:"GET",response_status:404,outcome:"unmatched",at:"2026-10-17T00:00:00Z",elapsed_ms:1}
  ];
  api.mockImplementation((_path:string,method?:string)=>{
    if(method==="DELETE"){calls=[];return Promise.resolve({ok:true});}
    return Promise.resolve(calls);
  });
  render(<Theme><MockCallActivity workspaceId="w" requestId="r"/></Theme>);
  await screen.findByText(/GET · 202 · Served/);
  expect(screen.getAllByRole("listitem")).toHaveLength(2);
  expect(screen.queryByText(/other/)).toBeNull();
  fireEvent.click(screen.getByRole("button",{name:"Clear workspace calls"}));
  expect(api).not.toHaveBeenCalledWith("/api/workspaces/w/mock-calls","DELETE");
  fireEvent.click(screen.getByRole("button",{name:"Confirm clear workspace calls"}));
  await waitFor(()=>expect(api).toHaveBeenCalledWith("/api/workspaces/w/mock-calls","DELETE"));
  await screen.findByText("No Mock calls yet");
});

it("withholds a previous account's loaded calls immediately when identity changes",async()=>{
  await setLanguage("en");
  api.mockImplementation(()=>Promise.resolve(auth.token==="owner"?[{id:"old",request_id:"r",example_id:"e",method:"GET",response_status:202,outcome:"served",at:"2026-10-17T00:00:00Z",elapsed_ms:4}]:[]));
  const page=render(<Theme><MockCallActivity workspaceId="w" requestId="r"/></Theme>);
  await screen.findByText(/GET · 202 · Served/);
  auth.token="other";
  page.rerender(<Theme><MockCallActivity workspaceId="w" requestId="r"/></Theme>);
  expect(screen.queryByText(/GET · 202 · Served/)).toBeNull();
  await screen.findByText("No Mock calls yet");
});

it("labels injected Mock faults separately from execution failures",async()=>{
  await setLanguage("en");
  api.mockResolvedValue([{id:"fault",request_id:"r",example_id:"e",method:"GET",response_status:503,outcome:"simulated_failure",at:"2026-10-17T00:00:00Z",elapsed_ms:7}]);
  render(<Theme><MockCallActivity workspaceId="w" requestId="r"/></Theme>);
  await screen.findByText(/GET · 503 · Simulated failure/);
});

it("labels simulated Mock rate limiting separately from execution failures",async()=>{
  await setLanguage("en");
  api.mockResolvedValue([{id:"limited",request_id:"r",example_id:"e",method:"GET",response_status:429,outcome:"rate_limited",at:"2026-10-17T00:00:00Z",elapsed_ms:2}]);
  render(<Theme><MockCallActivity workspaceId="w" requestId="r"/></Theme>);
  await screen.findByText(/GET · 429 · Rate limited/);
});
