import {useRef} from 'react';
import {api} from '../../shared/api';
import {LocalizedError} from '../../shared/i18n/errors';
import {useWorkbench} from '../workbench/context';
import type {useA2aCard} from './useA2aCard';
import type {A2aCardResult} from './types';
import A2aCardEditor from './A2aCardEditor';
export default function A2aCardDialog({open,onOpenChange,source}:{open:boolean;onOpenChange:(value:boolean)=>void;source:ReturnType<typeof useA2aCard>}) {
 const state=useWorkbench(),latest=useRef(state);latest.current=state;
 const scope=JSON.stringify([state.authenticated,state.accountId,state.draft?.id,state.request?.id,state.draft?.data.active_environment_id]);const current=useRef(scope);current.current=scope;
 if(state.request?.protocol?.kind!=='a2a')return null;
 const dialect=state.request.protocol.dialect;
 return <A2aCardEditor scope={scope} open={open} onOpenChange={onOpenChange} request={state.request} source={source.source} dark={state.dark} isCurrent={()=>current.current===scope&&latest.current.authenticated} onAttach={source.attach} beforeRead={state.dirty?async()=>{if(!(await state.save(true)))throw new LocalizedError('请先解决工作区保存冲突');}:undefined} importCard={text=>api<A2aCardResult>('/api/a2a/cards/import','POST',{...source.context(),source:text,dialect})} discoverCard={(url,discovery_id)=>api<A2aCardResult>('/api/a2a/cards/discover','POST',{...source.context(),discovery_id,request:{...state.request,url}})} cancelDiscovery={discovery_id=>api('/api/a2a/cards/discover/cancel','POST',{workspace_id:state.draft?.id,discovery_id})}/>;
}
