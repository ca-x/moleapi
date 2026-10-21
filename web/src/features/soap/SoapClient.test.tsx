// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { Theme } from "@radix-ui/themes";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { setLanguage } from "../../shared/i18n";
import { newRequest } from "../../shared/model";
import type { RequestSpec } from "../../shared/types";
import SoapClient from "./SoapClient";
const callbacks=vi.hoisted(()=>({editor:undefined as undefined|((v:string)=>void),definition:undefined as undefined|((v:string)=>void)}));
vi.mock("../../shared/ui",()=>({Editor:({value,label,onChange,readOnly}:{value:string;label:string;onChange?:(v:string)=>void;readOnly?:boolean})=>{callbacks.editor=onChange;return <textarea value={value} aria-label={label} readOnly={readOnly} onChange={e=>onChange?.(e.target.value)}/>;},Field:({label,children}:{label:string;children:React.ReactNode})=><label>{label}{children}</label>,Choice:({label,value,options,onChange,disabled}:{label:string;value:string;options:{value:string;label:string}[];onChange:(v:string)=>void;disabled?:boolean})=>{if(label==="WSDL 服务定义")callbacks.definition=onChange;return <select value={value} aria-label={label} disabled={disabled} onChange={e=>onChange(e.target.value)}>{options.map(o=><option key={o.value} value={o.value}>{o.label}</option>)}</select>;}}));
vi.mock("../requests/ResponsePane",()=>({ResponsePane:()=>null}));
function props(body="") {
  const request:RequestSpec={...newRequest(),body,body_kind:"text",method:"POST",protocol:{kind:"soap",version:"1.1",service:"Service",port:"Port",operation:"Echo",action:"Echo"}};
  return {scope:"member",request,specifications:[],schema:{services:[{name:"Service",ports:[{name:"Port",binding:"Binding",version:"1.1" as const,address:"http://fixture.test",operations:[{name:"Echo",action:"Echo",template:"<Envelope>template</Envelope>",fields:[]}]}]}]},dark:false,onChange:vi.fn(),isCurrent:()=>true,onImport:vi.fn(),onSend:vi.fn(),sendLabel:"发送草稿请求"};
}
beforeEach(async()=>{await setLanguage("zh-CN");});afterEach(cleanup);
it("generates a template as a canonical draft edit and never sends automatically",()=>{
 const state=props();render(<Theme><SoapClient {...state}/></Theme>);fireEvent.click(screen.getByRole("button",{name:"生成 XML 模板"}));
 expect(state.onChange).toHaveBeenCalledWith({body:"<Envelope>template</Envelope>",body_kind:"text",method:"POST"});expect(state.onSend).not.toHaveBeenCalled();
});
it("confirms replacing existing XML and rejects a newer XML draft while the dialog is open",async()=>{
 const state=props("<Envelope>my edit</Envelope>");const {rerender}=render(<Theme><SoapClient {...state}/></Theme>);
 fireEvent.click(screen.getByRole("button",{name:"生成 XML 模板"}));await screen.findByRole("alertdialog");expect(state.onChange).not.toHaveBeenCalled();
 rerender(<Theme><SoapClient {...state} request={{...state.request,body:"<Envelope>newer edit</Envelope>"}}/></Theme>);
 expect(screen.queryByRole("alertdialog")).toBeNull();expect(state.onChange).not.toHaveBeenCalled();
});
it("replacement requires explicit confirmation and current admission",async()=>{
 const state=props("<Envelope>edited</Envelope>");let allowed=true;state.isCurrent=()=>allowed;render(<Theme><SoapClient {...state}/></Theme>);
 fireEvent.click(screen.getByRole("button",{name:"生成 XML 模板"}));const dialog=await screen.findByRole("alertdialog");allowed=false;
 fireEvent.click(within(dialog).getByRole("button",{name:"替换"}));expect(state.onChange).not.toHaveBeenCalled();
});
it("retained XML and definition callbacks cannot target changed scope or draft",()=>{
 const state=props("original");const {rerender,unmount}=render(<Theme><SoapClient {...state}/></Theme>);const editor=callbacks.editor!,definition=callbacks.definition!;
 rerender(<Theme><SoapClient {...state} scope="new-account"/></Theme>);act(()=>{editor("late XML");definition("foreign-definition");});expect(state.onChange).not.toHaveBeenCalled();
 const disposed=callbacks.editor!;unmount();act(()=>disposed("disposed XML"));expect(state.onChange).not.toHaveBeenCalled();
});
it("CtrlEnter sends only the current admitted draft and honors source-save admission",()=>{
 const state=props("XML");const {rerender}=render(<Theme><SoapClient {...state}/></Theme>);const key=()=>fireEvent.keyDown(screen.getByRole("region",{name:"SOAP 客户端"}),{key:"Enter",ctrlKey:true});
 key();expect(state.onSend).toHaveBeenCalledTimes(1);rerender(<Theme><SoapClient {...state} sendDisabled/></Theme>);key();expect(state.onSend).toHaveBeenCalledTimes(1);
});
