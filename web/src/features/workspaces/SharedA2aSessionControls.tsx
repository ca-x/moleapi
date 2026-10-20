import {useState} from 'react';
import {Text} from '@radix-ui/themes';
import {t,useLanguage} from '../../shared/i18n';
import type {RequestSpec} from '../../shared/types';
import A2aClient,{type A2aChannel} from '../a2a/A2aClient';
export default function SharedA2aSessionControls(props:{request:RequestSpec;source?:string;channel:A2aChannel;dark:boolean;isCurrent:()=>boolean}) {
 useLanguage();const[request,setRequest]=useState(()=>structuredClone(props.request));return <section className="shared-a2a" aria-label={t('共享 A2A 实时调用')}><Text as="p" size="1" color="gray">{t('操作与参数修改仅用于当前连接，不会自动保存到共享请求。')}</Text><A2aClient scope={props.channel.session?.id??''} request={request} channel={props.channel} dark={props.dark} isCurrent={props.isCurrent} connectionSettings={false} source={props.source??(request.protocol?.kind==='a2a'?request.protocol.card_source??'':undefined)} onChange={patch=>{if(props.isCurrent())setRequest(current=>({...current,...patch}));}}/></section>;
}
