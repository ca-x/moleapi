import { useEffect, useRef, useState } from "react";
import { api } from "../../shared/api";
import { safeMessage } from "../../shared/model";
import { useWorkbench } from "../workbench/context";
import type { A2aCardResult } from "./types";
export function useA2aCard() {
  const state = useWorkbench();
  const latest = useRef(state); latest.current = state;
  const identity = JSON.stringify([state.authenticated,state.accountId,state.draft?.id,state.request?.id,state.draft?.data.active_environment_id,state.request?.protocol?.kind === "a2a" ? state.request.protocol.dialect : null]);
  const scope = useRef(identity), epoch = useRef(0);
  if (scope.current !== identity) { scope.current = identity; epoch.current++; }
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; epoch.current++; }; }, []);
  const [loaded,setLoaded] = useState<{scope:string;source:string;result:A2aCardResult}|null>(null);
  const [failure,setFailure] = useState<{scope:string;message:string}|null>(null);
  const [pending,setPending] = useState<string|null>(null);
  const config = state.request?.protocol?.kind === "a2a" ? state.request.protocol : null;
  const specification = state.draft?.data.specifications?.find(value=>value.id === state.request?.specification_id && value.kind === "a2a-agent-card");
  const source = specification?.source || config?.card_source || "";
  const card = loaded?.scope === identity && loaded.source === source ? loaded.result : null;
  function guard() {
    const origin = scope.current, generation = epoch.current;
    return () => mounted.current && latest.current.authenticated && scope.current === origin && epoch.current === generation;
  }
  function attach(result:A2aCardResult) {
    const origin = latest.current;
    const current = origin.request?.protocol;
    if (current?.kind !== "a2a") return;
    const existing = origin.draft?.data.specifications?.find(value=>value.kind === "a2a-agent-card" && value.source === result.specification.source);
    const candidate = existing || result.specification;
    if (!existing) origin.updateData(data=>({...data,specifications:[...(data.specifications || []),candidate]}));
    origin.updateRequest({specification_id:candidate.id,protocol:{...current,card_source:candidate.source,interface_url:null}});
    setLoaded({scope:scope.current,source:candidate.source,result}); setFailure(null);
  }
  function context() {
    const current = latest.current;
    if (!current.draft) throw new Error("工作区未加载");
    const collection = current.draft.data.collections.find(value=>value.requests.some(value=>value.id === current.request?.id))?.id;
    const environment = current.draft.data.active_environment_id;
    const locals = current.localVariables.values(current.draft,collection,environment);
    return {workspace_id:current.draft.id,environment_id:environment,...(locals.length ? {locals} : {})};
  }
  useEffect(() => {
    if (!source || !config || card) return;
    const current = guard(), origin = identity;
    let disposed = false;
    setPending(origin);
    void api<A2aCardResult>("/api/a2a/cards/import","POST",{...context(),source,dialect:config.dialect}).then(result=>{
      if (!disposed && current()) { setLoaded({scope:origin,source,result}); setFailure(null); }
    }).catch(caught=>{if(!disposed && current()) setFailure({scope:origin,message:safeMessage(caught)});}).finally(()=>{if(mounted.current)setPending(value=>value===origin?null:value);});
    return () => {disposed=true;};
  },[source,identity,config?.dialect,card]);
  return {card,specification,source,attach,guard,context,busy:pending===identity,error:failure?.scope===identity?failure.message:""};
}
