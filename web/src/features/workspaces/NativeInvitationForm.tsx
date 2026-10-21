import {useEffect,useRef,useState} from 'react';
import {Button,Callout,Flex,Text,TextArea} from '@radix-ui/themes';
import {memberApi,memberCredential} from '../../shared/memberApi';
import {t,translateCopy,useLanguage} from '../../shared/i18n';
import {errorCopy,LocalizedError,type ErrorCopy} from '../../shared/i18n/errors';
function invitation(input:string,server:string):string|null {
 try {const link=new URL(input.trim()),target=new URL(server);if(link.origin!==target.origin||link.username||link.password||link.pathname!=='/'||link.search)return null;const values=new URLSearchParams(link.hash.slice(1));const token=values.get('workspace_invite');return [...values.keys()].length===1&&token&&/^wi_[a-f0-9]{64}$/.test(token)?token:null;}catch{return null;}
}
export default function NativeInvitationForm(props:{scope:string;serverUrl:string;username:string;onJoined:()=>void}){
 useLanguage();const [input,setInput]=useState(''),[active,setActive]=useState<number|null>(null),[error,setError]=useState<{generation:number;value:ErrorCopy}|null>(null),[joined,setJoined]=useState<string|null>(null);
 const identity=JSON.stringify([props.scope,props.serverUrl,props.username,input]),latest=useRef(identity),epoch=useRef(0),mounted=useRef(true);if(latest.current!==identity){latest.current=identity;epoch.current++;}const generation=epoch.current,credential=memberCredential();
 useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
 function current(){return mounted.current&&latest.current===identity&&epoch.current===generation&&memberCredential()===credential&&!!credential;}
 function edit(value:string){if(latest.current!==identity)return;epoch.current++;setInput(value);setError(null);setJoined(null);}
 async function accept(){if(!current()||active===generation)return;const token=invitation(input,props.serverUrl);if(!token){setError({generation,value:errorCopy(new LocalizedError('请粘贴当前托管服务的有效工作区邀请链接。'))});return;}
  setActive(generation);setError(null);setJoined(null);
  try{await memberApi('/api/workspace-invitations/accept','POST',{token});if(current()){setInput('');setJoined(props.scope);props.onJoined();}}
  catch(cause){if(current())setError({generation,value:errorCopy(cause)});}
  finally{if(current())setActive(null);}
 }
 return <Flex direction="column" gap="2" asChild><section aria-label={t('接受托管邀请')}>
  <Text size="2">{t('当前托管账号：{{username}}', {username:props.username})}</Text>
  <Text size="1" color="gray" style={{overflowWrap:'anywhere'}}>{props.serverUrl}</Text>
  <TextArea aria-label={t('托管邀请链接')} placeholder={t('粘贴工作区邀请链接')} value={input} maxLength={4096} onChange={event=>edit(event.target.value)}/>
  <Text size="1" color="gray">{t('邀请只发送给当前连接的服务；不会复制工作区到本地。')}</Text>
  {error?.generation===generation&&<Callout.Root color="red" role="alert"><Callout.Text>{translateCopy(error.value)}</Callout.Text></Callout.Root>}
  {joined===props.scope&&<Text role="status">{t('已加入共享工作区')}</Text>}
  <div><Button loading={active===generation} disabled={!credential||!input.trim()||active===generation} onClick={()=>void accept()}>{t('接受托管邀请')}</Button></div>
 </section></Flex>;
}
