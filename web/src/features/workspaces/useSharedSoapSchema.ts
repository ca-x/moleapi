import { useCallback, useEffect, useRef, useState } from "react";
import { memberApi as api } from "../../shared/memberApi";
import { useLanguage, translateCopy } from "../../shared/i18n";
import { errorCopy, LocalizedError, type ErrorCopy } from "../../shared/i18n/errors";
import type { RequestSpec, Specification, Workspace, WorkspaceData } from "../../shared/types";
import type { SoapSchema, SoapSchemaResult } from "../soap/types";
import type { SoapImportInput } from "../soap/SoapSourceEditor";
export interface SharedSoapSourceContext { scope:string; base:string; request:RequestSpec; saved:Workspace; data:WorkspaceData; dirty:boolean; saving:boolean; conflict:boolean; isCurrent:()=>boolean; onAttach:(specification:Specification)=>string|undefined; }
export function useSharedSoapSchema(props:SharedSoapSourceContext) {
  useLanguage();const latest=useRef(props);latest.current=props;
  const identity=JSON.stringify([props.scope,props.request,props.saved.revision]);const active=useRef(identity);active.current=identity;
  const mounted=useRef(true);useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
  const [result,setResult]=useState<{scope:string;id:string;source:string;schema:SoapSchema}|null>(null);
  const [problem,setProblem]=useState<ErrorCopy>("");const [busy,setBusy]=useState(false);
  const guard=useCallback(()=>()=>mounted.current && active.current===identity && latest.current.isCurrent() && !latest.current.dirty && !latest.current.saving && !latest.current.conflict,[identity]);
  const specification=props.data.specifications?.find(s=>s.id===props.request.specification_id && s.kind==="wsdl");
  const schema=result?.scope===props.scope && result.id===specification?.id && result.source===specification.source ? result.schema : null;
  function attach(specification:Specification,schema:SoapSchema) {
    if(!guard()())return;const origin=latest.current,id=origin.onAttach(specification);
    if(id){setResult({scope:origin.scope,id,source:specification.source,schema});setProblem("");}
  }
  useEffect(()=>{
    if(props.dirty||props.saving||props.conflict||!specification||schema)return;
    const current=guard();let disposed=false;const origin=props.scope;setBusy(true);
    void api<SoapSchemaResult>(`${props.base}/soap/schema`,"POST",{request_id:props.request.id,specification_id:specification.id,expected_revision:props.saved.revision}).then(value=>{if(!disposed&&current()){setResult({scope:origin,id:specification.id,source:specification.source,schema:value.schema});setProblem("");}}).catch(error=>{if(!disposed&&current())setProblem(errorCopy(error));}).finally(()=>{if(!disposed&&mounted.current)setBusy(false);});
    return()=>{disposed=true;};
  },[props.scope,props.saved.revision,specification?.id,specification?.source,guard,props.dirty,props.saving,props.conflict,schema]);
  async function importSource(input:SoapImportInput) {
    const current=guard(),origin=latest.current;
    if(!current())throw new LocalizedError("先保存工作区，再导入或读取 SOAP 定义。");
    const value=await api<SoapSchemaResult>(`${origin.base}/soap/${"url" in input?"import-url":"import"}`,"POST",{request_id:origin.request.id,expected_revision:origin.saved.revision,...input});
    if(!current())throw new LocalizedError("SOAP 操作已取消");return value;
  }
  return {schema,specification,busy:busy&&!!specification&&!schema&&!props.dirty&&!props.saving&&!props.conflict,error:translateCopy(problem),guard,attach,importSource};
}
