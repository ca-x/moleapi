import { useRef } from "react";
import { message } from "../../shared/i18n";
import { LocalizedError } from "../../shared/i18n/errors";
import { useWorkbench } from "../workbench/context";
import type { RequestSpec } from "../../shared/types";
import { api } from "../../shared/api";
import McpConfigEditor from "./McpConfigEditor";
export default function McpConfigDialog({open,onOpenChange}:{open:boolean;onOpenChange:(open:boolean)=>void}) {
  const state=useWorkbench(),latest=useRef(state);latest.current=state;
  const scope=JSON.stringify([state.authenticated,state.accountId,state.draft?.id,state.request?.id,state.draft?.data.active_environment_id]);const identity=useRef(scope);identity.current=scope;
  function current(){return identity.current===scope&&latest.current.authenticated;}
  async function readSavedRequest(include_secrets:boolean,host_name:string){
    if(!state.draft||!state.request)throw new LocalizedError("请求尚未保存，请保存后重试");
    if(state.dirty&&!(await state.save(true)))throw new LocalizedError("请先解决保存冲突");
    if(!current())throw new LocalizedError("MCP 操作已取消");
    const value=await api<RequestSpec>(`/api/workspaces/${encodeURIComponent(state.draft.id)}/mcp/config/export`,"POST",{request_id:state.request.id,include_secrets,host_name});
    if(!current())throw new LocalizedError("MCP 操作已取消");return value;
  }
  if(state.request?.protocol?.kind!=="mcp")return null;
  return <McpConfigEditor scope={scope} open={open} onOpenChange={onOpenChange} request={state.request} dark={state.dark} isCurrent={current} onChange={patch=>latest.current.updateRequest(patch)} readSavedRequest={readSavedRequest} confirm={action=>state.setGuard({title:message("使用 MCP Host 配置"),description:message("当前连接配置与鉴权会被替换，请求参数草稿保留。导入后可检查配置，再手动连接。"),action})}/>;
}
