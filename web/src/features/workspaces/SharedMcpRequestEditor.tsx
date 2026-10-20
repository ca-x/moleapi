import { useRef, useState } from "react";
import { t, useLanguage } from "../../shared/i18n";
import { LocalizedError } from "../../shared/i18n/errors";
import { api } from "../../shared/api";
import type { RequestSpec } from "../../shared/types";
import McpClient from "../mcp/McpClient";
import McpConfigEditor from "../mcp/McpConfigEditor";
export default function SharedMcpRequestEditor(props:{scope:string;base:string;revision:number;request:RequestSpec;dark:boolean;disabled:boolean;exportDisabled:boolean;isCurrent:()=>boolean;onChange:(patch:Partial<RequestSpec>)=>void}) {
  useLanguage();const [open,setOpen]=useState(false);const identity=JSON.stringify([props.scope,props.request]),latest=useRef(props),current=useRef(identity);latest.current=props;current.current=identity;
  function admitted(){return current.current===identity&&!latest.current.disabled&&props.isCurrent();}
  async function readSavedRequest(include_secrets:boolean,host_name:string){
    if(!admitted()||latest.current.exportDisabled)throw new LocalizedError("先保存工作区，再导出 MCP 配置。");
    const value=await api<RequestSpec>(`${props.base}/mcp/config/export`,"POST",{request_id:props.request.id,expected_revision:props.revision,include_secrets,host_name});
    if(!admitted()||latest.current.exportDisabled)throw new LocalizedError("MCP 操作已取消");return value;
  }
  return <section className="shared-mcp" aria-label={t("共享 MCP 请求设置")}><McpClient scope={props.scope} request={props.request} dark={props.dark} disabled={props.disabled} isCurrent={admitted} onChange={props.onChange} onHost={()=>setOpen(true)}/><McpConfigEditor scope={props.scope} open={open} onOpenChange={setOpen} request={props.request} dark={props.dark} disabled={props.disabled} exportDisabled={props.exportDisabled} isCurrent={admitted} onChange={props.onChange} readSavedRequest={readSavedRequest}/></section>;
}
