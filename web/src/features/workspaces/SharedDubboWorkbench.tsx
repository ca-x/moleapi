import {useEffect,useRef,useState} from 'react';
import {Button,Flex,Text} from '@radix-ui/themes';
import { memberApi as api, memberCredential as token } from "../../shared/memberApi";
import {id} from '../../shared/model';
import {t,translateCopy,useLanguage} from '../../shared/i18n';
import {errorCopy,LocalizedError,type ErrorCopy} from '../../shared/i18n/errors';
import type {ApiResponse,RequestSpec,Workspace,WorkspaceData} from '../../shared/types';
import DubboWorkbench from '../dubbo/DubboWorkbench';
import type {Preview,MetadataDecision} from '../dubbo/MetadataPreviewPane';
import {ResponsePane} from '../requests/ResponsePane';
type Job={identity:string;generation:number;id:string;revision:number;request_id:string;base:string;credential:string|null;stopping:boolean};
export default function SharedDubboWorkbench(props:{scope:string;base:string;request:RequestSpec;saved:Workspace;data:WorkspaceData;dark:boolean;dirty:boolean;saving:boolean;conflict:boolean;isCurrent:()=>boolean;onChange:(patch:Partial<RequestSpec>)=>void;onCandidate:(workspace:Workspace)=>void;onExecuted?:()=>void}){
 useLanguage();const latest=useRef(props);latest.current=props;const identity=JSON.stringify([props.scope,props.request,props.data,props.dirty,props.saving,props.conflict]),scope=useRef(identity),epoch=useRef(0);if(scope.current!==identity){scope.current=identity;epoch.current++;}const generation=epoch.current;
 const mounted=useRef(true),currentScope=useRef(props.scope);currentScope.current=props.scope;useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
 const [capability,setCapability]=useState<{scope:string;available:boolean}|null>(null),[active,setActive]=useState<Job|null>(null),[failure,setFailure]=useState<{identity:string;value:ErrorCopy}|null>(null),[result,setResult]=useState<{identity:string;response:ApiResponse}|null>(null),[stopped,setStopped]=useState<string|null>(null),[stopping,setStopping]=useState(false);const activeJob=useRef<Job|null>(null),lastJob=useRef<string|null>(null);
 function admitted(){return mounted.current&&scope.current===identity&&epoch.current===generation&&!latest.current.saving&&!latest.current.conflict&&props.isCurrent();}
 const busy=active?.identity===identity&&active.generation===generation,available=capability?.scope===props.scope&&capability.available;
 const savedRequest=props.saved.data.collections.flatMap(c=>c.requests).find(r=>r.id===props.request.id&&r.protocol?.kind==='dubbo');
 useEffect(()=>{let disposed=false;const origin=props.scope;void api<{available:boolean}>('/api/dubbo/capability').then(value=>{if(!disposed&&mounted.current&&currentScope.current===origin&&latest.current.isCurrent())setCapability({scope:origin,available:value.available});}).catch(()=>{if(!disposed&&mounted.current&&currentScope.current===origin)setCapability({scope:origin,available:false});});return()=>{disposed=true;};},[props.scope]);
 useEffect(()=>()=>{const job=activeJob.current;if(job?.identity!==identity||job.generation!==generation)return;activeJob.current=null;if(token()===job.credential)void api(`${job.base}/dubbo/cancel`,'POST',{request_id:job.request_id,expected_revision:job.revision,job_id:job.id}).catch(()=>{});},[identity,generation]);
 function fields(){if(!admitted()||latest.current.dirty)throw new LocalizedError('先保存当前请求，再导入元数据。');return {request_id:props.request.id,expected_revision:props.saved.revision};}
 async function run(){
  if(!admitted()||activeJob.current||busy||!available||!savedRequest)return;const collection=props.saved.data.collections.find(c=>c.requests.some(r=>r.id===props.request.id));if(!collection)return;
  const job:Job={identity,generation,id:id(),revision:props.saved.revision,request_id:props.request.id,base:props.base,credential:token(),stopping:false};activeJob.current=job;lastJob.current=job.id;setActive(job);setFailure(null);setResult(null);setStopped(null);setStopping(false);
  try{const response=await api<ApiResponse>(props.dirty?`${props.base}/execute-draft`:`${props.base}/requests/${encodeURIComponent(props.request.id)}/execute`,'POST',props.dirty?{request:structuredClone(props.request),collection_id:collection.id,expected_workspace_revision:props.saved.revision,job_id:job.id}:{job_id:job.id});if(admitted()&&activeJob.current===job&&!job.stopping){setResult({identity,response});props.onExecuted?.();}}
  catch(error){if(admitted()&&activeJob.current===job&&!job.stopping)setFailure({identity,value:errorCopy(error)});}
  finally{if(activeJob.current===job){activeJob.current=null;if(admitted()){setActive(null);setStopping(false);}}}
 }
 async function stop(){const job=activeJob.current;if(!admitted()||!job||job.identity!==identity||job.generation!==generation||job.stopping||stopping||token()!==job.credential)return;job.stopping=true;setStopping(true);try{await api(`${props.base}/dubbo/cancel`,'POST',{request_id:job.request_id,expected_revision:job.revision,job_id:job.id});if(admitted()&&lastJob.current===job.id)setStopped(identity);}catch(error){if(admitted()&&lastJob.current===job.id){job.stopping=false;setFailure({identity,value:errorCopy(error)});}}finally{if(admitted()&&lastJob.current===job.id)setStopping(false);}}
 return <section className="shared-dubbo" aria-label={t('共享 Dubbo 客户端')} onKeyDown={event=>{if((event.ctrlKey||event.metaKey)&&event.key==='Enter'){event.preventDefault();void run();}}}>
  <Text as="p" size="1" role="status" color="gray">{capability?.scope!==props.scope?t('正在检查 Dubbo 运行能力…'):available?t('经典 Dubbo/Hessian2 调用已就绪。'):t('此实例尚未配置 Dubbo worker 和 Java；可先保存接口定义。')}</Text>
  {!savedRequest&&<Text as="p" size="1" color="gray">{t('先保存 Dubbo 请求，再调用服务。')}</Text>}
  <DubboWorkbench scope={props.scope} isCurrent={admitted} request={props.request} update={props.onChange} disabled={props.saving||props.conflict||busy} workspaceId={props.saved.id} workspaceData={props.data} workspaceRevision={props.saved.revision} dirty={props.dirty} onImported={candidate=>{if(!admitted()||latest.current.dirty||candidate.id!==props.saved.id||candidate.revision!==props.saved.revision)return;props.onCandidate(candidate);}} metadataOperations={{ready:!props.dirty&&!props.saving&&!props.conflict&&!!savedRequest,isCurrent:admitted,preview:()=>api<Preview>(`${props.base}/dubbo/metadata/preview`,'POST',fields()),importMetadata:(document_sha256:string,decisions:MetadataDecision[])=>api<Workspace>(`${props.base}/dubbo/metadata/import`,'POST',{...fields(),document_sha256,decisions})}}/>
  <Text as="p" size="1" color="gray">{t('元数据导入先暂存方法定义，点击保存后才更新共享工作区。')}</Text>
  <Flex gap="3" wrap="wrap"><Button disabled={!available||!savedRequest||busy||props.saving||props.conflict} loading={busy} onClick={()=>void run()}>{props.dirty?t('发送草稿请求'):t('发送已保存请求')}</Button>{busy&&<Button variant="soft" color="gray" loading={stopping} onClick={()=>void stop()}>{t('停止调用')}</Button>}</Flex>
  {stopped===identity&&<Text as="p" role="status">{t('已请求停止调用；远端结果可能未知。')}</Text>}
  <ResponsePane response={result?.identity===identity?result.response:null} error={failure?.identity===identity?translateCopy(failure.value):''} dark={props.dark} busy={busy}/>
 </section>;
}
