// @vitest-environment jsdom
import {cleanup,fireEvent,render,screen,waitFor,act} from "@testing-library/react";
import {Theme} from "@radix-ui/themes";
import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {setLanguage} from "../../shared/i18n";
import ProjectGenerationDialog from "./ProjectGenerationDialog";
const state=vi.hoisted(()=>({authenticated:true,accountId:"owner",draft:{id:"w",data:{specifications:[{id:"s",name:"Fixture",kind:"openapi",source:"spec",dialect:"3.0.3"}]}},dark:false,dirty:false,save:vi.fn()}));
const call=vi.hoisted(()=>vi.fn());
const pick=vi.hoisted(()=>vi.fn());
const nativeMode=vi.hoisted(()=>({value:false}));
const selectJava=vi.hoisted(()=>vi.fn());
vi.mock("./selectNativeJavaRuntime",()=>({selectNativeJavaRuntime:selectJava}));
vi.mock("../workbench/context",()=>({useWorkbench:()=>state}));vi.mock("../../shared/api",()=>({api:call,pickFile:pick,get native(){return nativeMode.value;}}));
vi.mock("../../shared/ui",async()=>{const real=await vi.importActual<object>("../../shared/ui");return {...real,Editor:({value}:{value:string})=><textarea aria-label="project-code" value={value} readOnly/>};});
beforeEach(async()=>{nativeMode.value=false;selectJava.mockReset();await setLanguage("en");vi.stubGlobal("ResizeObserver",class{observe(){}unobserve(){}disconnect(){}});state.accountId="owner";state.dirty=false;state.draft.data.specifications=[{id:"s",name:"Fixture",kind:"openapi",source:"spec",dialect:"3.0.3"}];call.mockReset();call.mockImplementation((path:string)=>path.endsWith("catalog")?Promise.resolve({java_available:false,targets:[{id:"rust-progenitor",kind:"client",upstream_stability:"stable",validation:"fixture",options:{packageName:"sdk"}},{id:"go-server",kind:"server",upstream_stability:"stable",options:{}}]}):Promise.resolve({engine:"progenitor",files:[{path:"src/lib.rs",encoding:"utf8",content:"generated-client",sha256:"hash",bytes:16,executable:false}],archive_base64:"AA==",source_sha256:"hash"}));});
afterEach(()=>{cleanup();vi.unstubAllGlobals();});
it("keeps native executable selection unavailable in hosted mode",async()=>{
 call.mockResolvedValue({java_available:false,targets:[{id:"go",kind:"client",upstream_stability:"stable",options:{}}]});
 render(<Theme><ProjectGenerationDialog open specificationId="s" onOpenChange={()=>{}}/></Theme>);
 await screen.findByText("This generator requires Java 17+ configured by the deployment owner. Native Rust SDK generation does not require Java.");
 expect(screen.queryByRole("button",{name:"Select Java…"})).toBeNull();
 expect(screen.getByRole("button",{name:"Generate project"}).hasAttribute("disabled")).toBe(true);
});
it("enables native generation immediately after the chosen runtime refreshes catalog",async()=>{
 nativeMode.value=true;let ready=false;
 selectJava.mockImplementation(async()=>{ready=true;return 21;});
 call.mockImplementation((path:string)=>path.endsWith("catalog")?Promise.resolve({java_available:ready,targets:[{id:"go",kind:"client",upstream_stability:"stable",options:{}}]}):Promise.resolve({engine:"fixture",files:[],archive_base64:"AA==",source_sha256:"hash"}));
 render(<Theme><ProjectGenerationDialog open specificationId="s" onOpenChange={()=>{}}/></Theme>);
 fireEvent.click(await screen.findByRole("button",{name:"Select Java…"}));
 await waitFor(()=>expect(screen.getByRole("button",{name:"Generate project"}).hasAttribute("disabled")).toBe(false));
 expect(screen.getByRole("button",{name:"Change Java…"})).toBeTruthy();
 fireEvent.click(screen.getByRole("button",{name:"Generate project"}));
 await waitFor(()=>expect(call).toHaveBeenCalledWith("/api/generation/projects","POST",expect.objectContaining({target:"go"})));
});
it("includes explicitly loaded templates in generation and hides them when the account changes",async()=>{
 const templates={format:"moleapi-codegen-templates-v1",files:[{path:"models.mustache",content:"{{classname}}"}]};pick.mockResolvedValue(JSON.stringify(templates));
 call.mockImplementation((path:string)=>path.endsWith("catalog")?Promise.resolve({java_available:true,targets:[{id:"typescript-fetch",kind:"client",upstream_stability:"stable",options:{}}]}):Promise.resolve({engine:"openapi-generator",source_sha256:"hash",archive_base64:"AA==",files:[{path:"models/Pet.ts",encoding:"utf8",content:"generated-model",sha256:"h",bytes:15}]}));
 const view=render(<Theme><ProjectGenerationDialog open specificationId="s" onOpenChange={()=>{}}/></Theme>);
 await screen.findByText("Custom Mustache templates");fireEvent.click(screen.getByText("Custom Mustache templates"));fireEvent.click(screen.getByRole("button",{name:"Load template JSON / generation snapshot"}));
 await screen.findByDisplayValue("{{classname}}");
 fireEvent.click(screen.getByRole("button",{name:"Generate project"}));
 await waitFor(()=>expect(call).toHaveBeenCalledWith("/api/generation/projects","POST",expect.objectContaining({target:"typescript-fetch",templates})));
 state.accountId="next-owner";view.rerender(<Theme><ProjectGenerationDialog open specificationId="s" onOpenChange={()=>{}}/></Theme>);expect(screen.queryByDisplayValue("{{classname}}")).toBeNull();
});
it("shows real project files and switches language without changing generation state",async()=>{
 render(<Theme><ProjectGenerationDialog open specificationId="s" onOpenChange={()=>{}}/></Theme>);await waitFor(()=>expect(screen.getByRole("button",{name:"Generate project"}).getAttribute("disabled")).toBeNull());fireEvent.click(screen.getByRole("button",{name:"Generate project"}));await screen.findByDisplayValue("generated-client");expect(call).toHaveBeenCalledWith("/api/generation/projects","POST",expect.objectContaining({workspace_id:"w",specification_id:"s",target:"rust-progenitor",include_secrets:false}));
 await act(()=>setLanguage("zh-CN"));expect(screen.getByRole("button",{name:"下载项目 ZIP"})).toBeTruthy();expect(screen.getByDisplayValue("generated-client")).toBeTruthy();
});
it("close and reopen discard a late project reply even when source and target are identical",async()=>{
 let finish!:(value:unknown)=>void;call.mockImplementation((path:string)=>path.endsWith("catalog")?Promise.resolve({java_available:false,targets:[{id:"rust-progenitor",kind:"client",upstream_stability:"stable",options:{}}]}):path.endsWith("cancel")?Promise.resolve({cancelled:true}):new Promise(resolve=>{finish=resolve;}));
 const view=render(<Theme><ProjectGenerationDialog open specificationId="s" onOpenChange={()=>{}}/></Theme>);await waitFor(()=>expect(screen.getByRole("button",{name:"Generate project"}).getAttribute("disabled")).toBeNull());fireEvent.click(screen.getByRole("button",{name:"Generate project"}));await waitFor(()=>expect(finish).toBeTypeOf("function"));
 view.rerender(<Theme><ProjectGenerationDialog open={false} specificationId="s" onOpenChange={()=>{}}/></Theme>);view.rerender(<Theme><ProjectGenerationDialog open specificationId="s" onOpenChange={()=>{}}/></Theme>);
 await act(async()=>finish({engine:"old",source_sha256:"old",archive_base64:"AA==",files:[{path:"old.rs",encoding:"utf8",content:"old-private-project",sha256:"h",bytes:19}]}));expect(screen.queryByDisplayValue("old-private-project")).toBeNull();expect(call.mock.calls.some(([path])=>path==="/api/generation/projects/cancel")).toBe(true);
});
it("submits typed Kotlin union controls and preserves them across locale changes",async()=>{
 call.mockImplementation((path:string)=>path.endsWith("catalog")?Promise.resolve({java_available:true,targets:[{id:"kotlin",kind:"client",upstream_stability:"stable",options:{library:"jvm-okhttp4",serializationLibrary:"moshi",generateOneOfAnyOfWrappers:false}}]}):Promise.resolve({engine:"fixture",files:[],archive_base64:"AA==",source_sha256:"hash"}));
 render(<Theme><ProjectGenerationDialog open specificationId="s" onOpenChange={()=>{}}/></Theme>);
 const checkbox=await screen.findByRole("checkbox",{name:"Generate union wrappers"});
 expect(checkbox.getAttribute("aria-checked")).toBe("false");
 fireEvent.click(checkbox);
 const fields=screen.getAllByRole("textbox");
 fireEvent.change(fields.find(field=>field.getAttribute("placeholder")==="jvm-okhttp4")!,{target:{value:"jvm-retrofit2"}});
 fireEvent.change(fields.find(field=>field.getAttribute("placeholder")==="moshi")!,{target:{value:"gson"}});
 await act(()=>setLanguage("zh-CN"));
 expect(screen.getByRole("checkbox",{name:"生成联合类型包装"}).getAttribute("aria-checked")).toBe("true");
 fireEvent.click(screen.getByRole("button",{name:"生成项目"}));
 await waitFor(()=>expect(call).toHaveBeenCalledWith("/api/generation/projects","POST",expect.objectContaining({target:"kotlin",options:{library:"jvm-retrofit2",serializationLibrary:"gson",generateOneOfAnyOfWrappers:true}})));
});

it("blocks Go messages without the required plugin and keeps the explanation localized",async()=>{
 state.draft.data.specifications[0].kind="protobuf";
 call.mockImplementation(()=>Promise.resolve({java_available:false,protoc_available:true,protobuf_plugins:[],grpc_plugins:["go"],targets:[{id:"protobuf-go",kind:"protobuf",upstream_stability:"stable",options:{}}]}));
 const view=render(<Theme><ProjectGenerationDialog open specificationId="s" onOpenChange={()=>{}}/></Theme>);
 const alert=await screen.findByRole("alert");expect(alert.textContent).toContain("Go message plugin");expect(screen.getByText("Generate messages or gRPC interfaces from original Protobuf definitions. Existing code is preserved; generated output is neither executed nor published.")).toBeTruthy();
 const generate=screen.getByRole("button",{name:"Generate project"});expect(generate.getAttribute("disabled")).not.toBeNull();fireEvent.click(generate);
 expect(call.mock.calls.filter(([path])=>path==="/api/generation/projects")).toHaveLength(0);
 await act(()=>setLanguage("zh-CN"));expect(screen.getByRole("alert").textContent).toContain("Go 消息插件");expect(screen.getByRole("button",{name:"生成项目"}).getAttribute("disabled")).not.toBeNull();
 view.rerender(<Theme><ProjectGenerationDialog open={false} specificationId="s" onOpenChange={()=>{}}/></Theme>);
 call.mockImplementation(()=>Promise.resolve({java_available:false,protoc_available:true,protobuf_plugins:["go"],grpc_plugins:["go"],targets:[{id:"protobuf-go",kind:"protobuf",upstream_stability:"stable",options:{}}]}));
 view.rerender(<Theme><ProjectGenerationDialog open specificationId="s" onOpenChange={()=>{}}/></Theme>);
 await waitFor(()=>expect(screen.getByRole("button",{name:"生成项目"}).getAttribute("disabled")).toBeNull());expect(screen.queryByRole("alert")).toBeNull();
});
it("allows explicitly configured Go message-only generation with a visible gRPC prerequisite",async()=>{
 state.draft.data.specifications[0].kind="protobuf";
 call.mockImplementation((path:string)=>path.endsWith("catalog")?Promise.resolve({java_available:false,protoc_available:true,protobuf_plugins:["go"],grpc_plugins:[],targets:[{id:"protobuf-go",kind:"protobuf",upstream_stability:"stable",options:{}}]}):Promise.resolve({engine:"fixture",files:[],archive_base64:"AA==",source_sha256:"hash"}));
 render(<Theme><ProjectGenerationDialog open specificationId="s" onOpenChange={()=>{}}/></Theme>);
 await screen.findByText("This target currently generates Protobuf message types; gRPC service interfaces require the corresponding language plugin.");
 const generate=screen.getByRole("button",{name:"Generate project"});expect(generate.getAttribute("disabled")).toBeNull();fireEvent.click(generate);
 await waitFor(()=>expect(call).toHaveBeenCalledWith("/api/generation/projects","POST",expect.objectContaining({target:"protobuf-go"})));
});
it("blocks Kotlin coroutine generation when its Java service helper is absent",async()=>{
 state.draft.data.specifications[0].kind="protobuf";
 call.mockImplementation(()=>Promise.resolve({java_available:false,protoc_available:true,grpc_plugins:["kotlin"],targets:[{id:"protobuf-kotlin",kind:"protobuf",upstream_stability:"stable",options:{}}]}));
 render(<Theme><ProjectGenerationDialog open specificationId="s" onOpenChange={()=>{}}/></Theme>);
 expect((await screen.findByRole("alert")).textContent).toContain("both Kotlin and Java plugins");expect(screen.getByRole("button",{name:"Generate project"}).getAttribute("disabled")).not.toBeNull();
});
