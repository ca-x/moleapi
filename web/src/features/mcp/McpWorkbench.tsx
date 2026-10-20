import { useEffect, useRef, useState } from "react";
import { useWorkbench } from "../workbench/context";
import McpClient from "./McpClient";
import McpConfigDialog from "./McpConfigDialog";
export default function McpWorkbench() {
  const state=useWorkbench(),latest=useRef(state);latest.current=state;
  const scope=JSON.stringify([state.authenticated,state.accountId,state.draft?.id,state.request?.id,state.draft?.data.active_environment_id]);
  const current=useRef(scope);current.current=scope;const [open,setOpen]=useState(false);useEffect(()=>{setOpen(false);},[scope]);
  if(state.request?.protocol?.kind!=="mcp")return null;
  return <><McpClient key={scope} scope={scope} request={state.request} dark={state.dark} channel={state.protocolSession} runRef={state.mcpRun} isCurrent={()=>current.current===scope&&latest.current.authenticated} onChange={patch=>latest.current.updateRequest(patch)} onHost={()=>setOpen(true)}/><McpConfigDialog open={open} onOpenChange={setOpen}/></>;
}
