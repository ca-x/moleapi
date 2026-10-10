// @vitest-environment jsdom
import {act,cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {api,token} from "../../shared/api";
import {initialData} from "../../shared/model";
import {useWorkbench} from "../workbench/context";
import {saveProjectFile} from "../generation/saveProjectFile";
import CiPanel from "./CiPanel";
vi.mock("../../shared/api",()=>({api:vi.fn(),native:false,token:vi.fn()}));
vi.mock("../workbench/context",()=>({useWorkbench:vi.fn()}));
vi.mock("../generation/saveProjectFile",()=>({saveProjectFile:vi.fn()}));
vi.mock("../../shared/ui",()=>({Editor:({value,label}:{value:string;label:string})=><textarea aria-label={label} readOnly value={value}/>,Field:({label,children}:{label:string;children:React.ReactNode})=><label>{label}{children}</label>,Choice:({label,value,options,onChange}:{label:string;value:string;options:{value:string;label:string}[];onChange:(value:string)=>void})=><label>{label}<select value={value} onChange={event=>onChange(event.target.value)}>{options.map(option=><option key={option.value} value={option.value}>{option.label}</option>)}</select></label>}));
let state:ReturnType<typeof useWorkbench>;
const preset={filename:"moleapi-tests.yml",mime:"application/yaml",content:"pipeline-content",command:[],prerequisites:[]};
beforeEach(()=>{vi.mocked(api).mockReset();vi.mocked(saveProjectFile).mockReset();vi.mocked(token).mockReturnValue("session");state={accountId:"owner",authenticated:true,draft:{id:"w",revision:3,data:initialData()},dirty:false,dark:false} as unknown as ReturnType<typeof useWorkbench>;vi.mocked(useWorkbench).mockImplementation(()=>state);});
afterEach(cleanup);
it("generates from saved revision without credential values and fences the file picker",async()=>{
 vi.mocked(api).mockResolvedValue(preset);const view=render(<Theme><CiPanel/></Theme>);
 fireEvent.click(screen.getAllByRole("checkbox")[0]);
 fireEvent.click(screen.getByRole("button",{name:"生成 CI 配置"}));await screen.findByLabelText("CI 配置预览");
 expect(api).toHaveBeenCalledWith("/api/workspaces/w/ci-preset","POST",expect.objectContaining({expected_revision:3,config:expect.objectContaining({source:{kind:"remote",server:window.location.origin,workspace:"w"},notifications:{mode:"silent"},requests:[state.draft!.data.collections[0].requests[0].id]})}));expect(JSON.stringify(vi.mocked(api).mock.calls)).not.toContain('"session"');
 fireEvent.click(screen.getByRole("button",{name:"下载 CI 配置"}));await waitFor(()=>expect(saveProjectFile).toHaveBeenCalled());const current=vi.mocked(saveProjectFile).mock.calls[0][2];expect(current()).toBe(true);
 state={...state,accountId:"other"};view.rerender(<Theme><CiPanel/></Theme>);expect(current()).toBe(false);expect(screen.queryByLabelText("CI 配置预览")).toBeNull();
});
it("discards late generation after a credential change and does not generate dirty state",async()=>{
 let resolve!:(value:unknown)=>void;vi.mocked(api).mockImplementation(()=>new Promise(done=>{resolve=done;}));const view=render(<Theme><CiPanel/></Theme>);
 fireEvent.click(screen.getByRole("button",{name:"生成 CI 配置"}));await waitFor(()=>expect(api).toHaveBeenCalled());vi.mocked(token).mockReturnValue("new-session");view.rerender(<Theme><CiPanel/></Theme>);await act(async()=>resolve(preset));expect(screen.queryByLabelText("CI 配置预览")).toBeNull();
 state={...state,dirty:true};view.rerender(<Theme><CiPanel/></Theme>);expect((screen.getByRole("button",{name:"生成 CI 配置"}) as HTMLButtonElement).disabled).toBe(true);
});
it("exports the privacy-projected native source separately and rejects a changed revision",async()=>{
 vi.mocked(api).mockResolvedValue({filename:"source.json",mime:"application/json",content:JSON.stringify(state.draft)});render(<Theme><CiPanel/></Theme>);fireEvent.change(screen.getByLabelText("运行来源"),{target:{value:"file"}});fireEvent.click(screen.getByRole("button",{name:"导出 CI 集合文件"}));await waitFor(()=>expect(saveProjectFile).toHaveBeenCalled());expect(api).toHaveBeenCalledWith("/api/workspaces/w/export","POST",{format:"moleapi",include_secrets:false});
 vi.mocked(saveProjectFile).mockClear();vi.mocked(api).mockResolvedValue({filename:"source.json",content:JSON.stringify({...state.draft,revision:4})});fireEvent.click(screen.getByRole("button",{name:"导出 CI 集合文件"}));await screen.findByRole("alert");expect(saveProjectFile).not.toHaveBeenCalled();
});
