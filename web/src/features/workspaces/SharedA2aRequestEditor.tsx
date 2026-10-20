import {useEffect,useRef,useState} from 'react';
import {api} from '../../shared/api';
import {LocalizedError,errorCopy,type ErrorCopy} from '../../shared/i18n/errors';
import {t,translateCopy,useLanguage} from '../../shared/i18n';
import type {RequestSpec,WorkspaceData} from '../../shared/types';
import type {A2aCardResult} from '../a2a/types';
import A2aClient from '../a2a/A2aClient';
import A2aCardEditor from '../a2a/A2aCardEditor';
export default function SharedA2aRequestEditor(props:{scope:string;base:string;revision:number;request:RequestSpec;data:WorkspaceData;dark:boolean;disabled:boolean;sourcesDisabled:boolean;isCurrent:()=>boolean;onChange:(patch:Partial<RequestSpec>)=>void;onAttach:(result:A2aCardResult)=>void}) {
 useLanguage();const [open,setOpen]=useState(false),[loaded,setLoaded]=useState<{source:string;version:string;result:A2aCardResult}|null>(null),[pending,setPending]=useState<string|null>(null),[error,setError]=useState<ErrorCopy>('');
 const latest=useRef(props);latest.current=props;const identity=JSON.stringify([props.scope,props.request,props.sourcesDisabled]),current=useRef(identity);current.current=identity;const mounted=useRef(true);useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
 const config=props.request.protocol?.kind==='a2a'?props.request.protocol:null,spec=props.data.specifications?.find(s=>s.id===props.request.specification_id&&s.kind==='a2a-agent-card'),source=spec?.source??config?.card_source??'';
 function admitted(){return mounted.current&&current.current===identity&&!latest.current.disabled&&props.isCurrent();}
 function fields(){if(!admitted()||latest.current.sourcesDisabled)throw new LocalizedError('先保存工作区，再读取 Agent Card。');return {request_id:props.request.id,expected_revision:props.revision};}
 const card=loaded?.source===source&&loaded.version===config?.dialect?loaded.result:null;
 useEffect(()=>{
  if(!source||!config||!spec||props.sourcesDisabled||card)return;const origin=identity;let disposed=false;setPending(origin);
  void api<A2aCardResult>(`${props.base}/a2a/cards/read`,'POST',{...fields(),specification_id:spec.id}).then(result=>{if(!disposed&&admitted()){setLoaded({source,version:config.dialect,result});setError('');}}).catch(caught=>{if(!disposed&&admitted())setError(errorCopy(caught));}).finally(()=>{if(mounted.current)setPending(value=>value===origin?null:value);});
  return()=>{disposed=true;};
 },[identity,source,spec?.id,card]);
 return <section className="shared-a2a" aria-label={t('共享 A2A 请求设置')}><A2aClient scope={props.scope} request={props.request} dark={props.dark} isCurrent={admitted} disabled={props.disabled} onChange={props.onChange} card={card} source={source} sourceBusy={pending===identity} sourceError={translateCopy(error)} onSource={()=>setOpen(true)}/>{props.sourcesDisabled&&<p>{t('先保存工作区，再读取 Agent Card。')}</p>}<A2aCardEditor scope={props.scope} open={open} onOpenChange={setOpen} request={props.request} source={source} dark={props.dark} disabled={props.disabled||props.sourcesDisabled} isCurrent={admitted} onAttach={result=>{if(!admitted())return;setLoaded({source:result.specification.source,version:result.dialect,result});props.onAttach(result);}} importCard={text=>api<A2aCardResult>(`${props.base}/a2a/cards/import`,'POST',{...fields(),source:text})} discoverCard={(url,discovery_id)=>api<A2aCardResult>(`${props.base}/a2a/cards/discover`,'POST',{...fields(),url,discovery_id})} cancelDiscovery={discovery_id=>api(`${props.base}/a2a/cards/discover/cancel`,'POST',{request_id:props.request.id,expected_revision:props.revision,discovery_id})}/></section>;
}
