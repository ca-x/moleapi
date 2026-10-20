import { useEffect, useRef, useState } from "react";
import { useWorkbench } from "../workbench/context";
import { useSoapSchema } from "./useSoapSchema";
import SoapClient from "./SoapClient";
import SoapSourceDialog from "./SoapSourceDialog";
export default function SoapWorkbench() {
  const state=useWorkbench(),source=useSoapSchema(),latest=useRef(state);latest.current=state;
  const scope=JSON.stringify([state.authenticated,state.accountId,state.draft?.id,state.request?.id,state.draft?.data.active_environment_id]);
  const [open,setOpen]=useState(false);useEffect(()=>{setOpen(false);},[scope]);
  if(state.request?.protocol?.kind!=="soap")return null;
  return <><SoapClient key={scope} scope={scope} request={state.request} specifications={state.draft?.data.specifications??[]} schema={source.schema} dark={state.dark} onChange={patch=>latest.current.updateRequest(patch)} isCurrent={source.guard()} onImport={()=>setOpen(true)} sourceError={source.error} sourceBusy={source.busy} response={state.response} requestError={state.requestError} sending={state.sending}/><SoapSourceDialog open={open} onOpenChange={setOpen} source={source}/></>;
}
