// @vitest-environment jsdom
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { api } from "../../shared/api";
import { initialData, newRequest } from "../../shared/model";
import { setLanguage } from "../../shared/i18n";
import type { Specification, Workspace } from "../../shared/types";
import { useSharedSoapSchema, type SharedSoapSourceContext } from "./useSharedSoapSchema";
vi.mock("../../shared/api",()=>({api:vi.fn()}));
const base="/api/owners/owner/workspaces/w";
const spec:Specification={id:"wsdl",name:"Saved WSDL",kind:"wsdl",dialect:"wsdl1.1",source:'{"entry_file":"service.wsdl","files":[]}'};
const schema={services:[]};
function context(attach=vi.fn().mockReturnValue("wsdl")):SharedSoapSourceContext {
  const data=initialData(),request={...newRequest(),id:"r",method:"POST",protocol:{kind:"soap" as const,version:"1.1" as const,service:"",port:"",operation:"",action:""}};data.collections[0].requests=[request];
  const saved:Workspace={id:"w",name:"Shared",revision:1,updated_at:"now",data};
  return {scope:"member",base,request,saved,data,dirty:false,saving:false,conflict:false,isCurrent:()=>true,onAttach:attach};
}
beforeEach(async()=>{await setLanguage("zh-CN");vi.mocked(api).mockReset();});afterEach(cleanup);
it("imports source and explicit URL candidates only through saved request-bound member routes, without CAS",async()=>{
  const attach=vi.fn().mockReturnValue("wsdl"),props=context(attach);vi.mocked(api).mockResolvedValue({specification:spec,schema});
  const {result}=renderHook(()=>useSharedSoapSchema(props));
  await act(async()=>{const candidate=await result.current.importSource({name:"Bundle",source:spec.source});result.current.attach(candidate.specification,candidate.schema);});
  expect(api).toHaveBeenCalledWith(`${base}/soap/import`,"POST",{request_id:"r",expected_revision:1,name:"Bundle",source:spec.source});expect(attach).toHaveBeenCalledWith(spec);
  await result.current.importSource({name:"URL",url:"https://fixture.test/?wsdl",verify_tls:true,timeout_ms:30000});
  expect(api).toHaveBeenCalledWith(`${base}/soap/import-url`,"POST",{request_id:"r",expected_revision:1,name:"URL",url:"https://fixture.test/?wsdl",verify_tls:true,timeout_ms:30000});
  expect(vi.mocked(api).mock.calls.every(([,method])=>method!=="PUT")).toBe(true);
});
it.each(["request","scope","revision","dirty","saving","conflict","admission"] as const)("withholds a late source candidate after %s changes",async(boundary)=>{
  const attach=vi.fn().mockReturnValue("wsdl");let props=context(attach),admitted=true;props.isCurrent=()=>admitted;
  let finish!:(value:unknown)=>void;vi.mocked(api).mockImplementation(()=>new Promise(resolve=>{finish=resolve;}));
  const {result,rerender}=renderHook(input=>useSharedSoapSchema(input),{initialProps:props});
  const work=result.current.importSource({name:"Source",source:spec.source});const checked=expect(work).rejects.toThrow("已取消");
  if(boundary==="admission")admitted=false;
  else {props={...props,...(boundary==="request"?{request:{...props.request,body:"new XML"}}:{}),...(boundary==="scope"?{scope:"new-account"}:{}),...(boundary==="revision"?{saved:{...props.saved,revision:2}}:{}),...(boundary==="dirty"?{dirty:true}:{}),...(boundary==="saving"?{saving:true}:{}),...(boundary==="conflict"?{conflict:true}:{})};rerender(props);}
  await act(async()=>finish({specification:spec,schema}));await checked;expect(attach).not.toHaveBeenCalled();
});
it("retained attach and import callbacks cannot retarget a newer scope",async()=>{
  const attach=vi.fn().mockReturnValue("wsdl"),props=context(attach);const {result,rerender}=renderHook(input=>useSharedSoapSchema(input),{initialProps:props});const retained=result.current;
  rerender({...props,scope:"other"});act(()=>retained.attach(spec,schema));await expect(retained.importSource({name:"late",source:spec.source})).rejects.toThrow("先保存");expect(attach).not.toHaveBeenCalled();expect(api).not.toHaveBeenCalled();
});
it("uses only the saved request association and retries a withheld schema read after edit/revert",async()=>{
  const props=context();props.request={...props.request,specification_id:"wsdl"};props.data.specifications=[spec];let finish!:(value:unknown)=>void;
  vi.mocked(api).mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve;})).mockResolvedValue({specification:spec,schema});
  const {result,rerender}=renderHook(input=>useSharedSoapSchema(input),{initialProps:props});rerender({...props,dirty:true});await act(async()=>finish({specification:spec,schema}));expect(result.current.schema).toBeNull();expect(result.current.busy).toBe(false);
  rerender(props);await waitFor(()=>expect(result.current.schema).toEqual(schema));await waitFor(()=>expect(result.current.busy).toBe(false));
  expect(api).toHaveBeenCalledTimes(2);expect(api).toHaveBeenLastCalledWith(`${base}/soap/schema`,"POST",{request_id:"r",specification_id:"wsdl",expected_revision:1});
});
