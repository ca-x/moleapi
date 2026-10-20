import { useEffect, useRef, useState } from "react";
import { useWorkbench } from "../workbench/context";
import { useA2aCard } from "./useA2aCard";
import A2aCardDialog from "./A2aCardDialog";
import A2aClient from "./A2aClient";
export default function A2aWorkbench(){
 const state=useWorkbench(),source=useA2aCard(),latest=useRef(state);latest.current=state;
 const scope=JSON.stringify([state.authenticated,state.accountId,state.draft?.id,state.request?.id,state.draft?.data.active_environment_id]);const current=useRef(scope);current.current=scope;
 const [open,setOpen]=useState(false);useEffect(()=>setOpen(false),[scope]);
 if(state.request?.protocol?.kind!=="a2a")return null;
 return <><A2aClient key={scope} scope={scope} request={state.request} dark={state.dark} isCurrent={()=>current.current===scope&&latest.current.authenticated} onChange={patch=>latest.current.updateRequest(patch)} channel={state.protocolSession} runRef={state.a2aRun} card={source.card} source={source.source} sourceBusy={source.busy} sourceError={source.error} confirm={guard=>state.setGuard(guard)} onSource={()=>setOpen(true)}/><A2aCardDialog open={open} onOpenChange={setOpen} source={source}/></>;
}
