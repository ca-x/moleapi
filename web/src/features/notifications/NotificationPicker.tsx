import {useQuery} from "@tanstack/react-query";
import {Checkbox,Flex,Text} from "@radix-ui/themes";
import {api} from "../../shared/api";
import {t,useLanguage} from "../../shared/i18n";
import {useWorkbench} from "../workbench/context";
import type {NotificationTarget} from "../../shared/types";
export default function NotificationPicker({value,onChange,disabled,label}:{value:string[];onChange:(value:string[])=>void;disabled:boolean;label:string}){
 useLanguage();const state=useWorkbench(),workspace=state.draft?.id??"";
 const targets=useQuery({queryKey:["notification-targets",state.accountId,workspace],queryFn:()=>api<NotificationTarget[]>(`/api/workspaces/${workspace}/notifications`),enabled:state.authenticated&&!!workspace});
 return <Flex direction="column" gap="2"><Text size="2">{label}</Text><Flex gap="3" wrap="wrap">{targets.data?.map(target=><label key={target.id} className="checkbox-label"><Checkbox disabled={disabled} checked={value.includes(target.id)} onCheckedChange={checked=>onChange(checked===true?[...value,target.id]:value.filter(id=>id!==target.id))}/>{target.settings.name}{!target.settings.enabled&&` · ${t("已停用")}`}</label>)}</Flex>{value.some(id=>!targets.data?.some(target=>target.id===id))&&targets.data&&<Text size="1" color="orange">{t("部分通知对象不可用，请重新选择。")}</Text>}</Flex>;
}
