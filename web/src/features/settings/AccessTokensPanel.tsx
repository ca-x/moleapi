import {useEffect,useId,useRef,useState} from "react";
import {useQuery,useQueryClient} from "@tanstack/react-query";
import {AlertDialog,Button,Callout,Flex,Heading,Text,TextArea,TextField} from "@radix-ui/themes";
import {api,token} from "../../shared/api";
import {t,useLanguage} from "../../shared/i18n";
import {liveError} from "../../shared/i18n/errors";
import {Choice,Field} from "../../shared/ui";
interface AccessToken {id:string;name:string;created_at:number;expires_at:number;expired:boolean}
export default function AccessTokensPanel({accountId}:{accountId:string}) {
 const instance=useId();const {language}=useLanguage();const client=useQueryClient(),credential=token();
 const boundary=useRef({accountId,credential,epoch:0});
 if(boundary.current.accountId!==accountId||boundary.current.credential!==credential) boundary.current={accountId,credential,epoch:boundary.current.epoch+1};
 const scope=JSON.stringify([accountId,boundary.current.epoch]);const latest=useRef(scope);latest.current=scope;
 const mounted=useRef(true);useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
 const [name,setName]=useState(""),[days,setDays]=useState("90"),[pending,setPending]=useState(false),[error,setError]=useState<ReturnType<typeof liveError>>("");
 const [secret,setSecret]=useState<{scope:string;value:string}|null>(null),[copied,setCopied]=useState(false),[selected,setSelected]=useState<AccessToken|null>(null);
 useEffect(()=>{setSecret(null);setCopied(false);setSelected(null);setError("");setPending(false);setName("");},[scope]);
 const queryKey=["access-tokens",accountId,instance,boundary.current.epoch];
 const tokens=useQuery({queryKey,queryFn:()=>api<AccessToken[]>("/api/auth/tokens"),enabled:!!credential,retry:false});
 const current=(origin:string)=>mounted.current&&latest.current===origin&&token()===credential;
 async function create(event:React.FormEvent) {
  event.preventDefault();if(pending||!current(scope))return;const origin=scope;setPending(true);setError("");setSecret(null);setCopied(false);
  try {const value=await api<AccessToken&{token:string}>("/api/auth/tokens","POST",{name:name.trim(),expires_in_days:Number(days)});if(current(origin)){setSecret({scope:origin,value:value.token});setName("");void client.invalidateQueries({queryKey});}}
  catch(error){if(current(origin))setError(liveError(error));}finally{if(current(origin))setPending(false);}
 }
 async function revoke() {
  if(pending||!selected||!current(scope))return;const origin=scope;setPending(true);setError("");
  try{await api(`/api/auth/tokens/${encodeURIComponent(selected.id)}`,"DELETE");if(current(origin)){setSelected(null);setSecret(null);void client.invalidateQueries({queryKey});}}
  catch(error){if(current(origin))setError(liveError(error));}finally{if(current(origin))setPending(false);}
 }
 async function copy(){if(secret?.scope!==scope||!current(scope))return;const origin=scope;try{await navigator.clipboard.writeText(secret.value);if(current(origin))setCopied(true);}catch(error){if(current(origin))setError(liveError(error));}}
 const date=(value:number)=>new Date(value*1000).toLocaleString(language);
 return <section aria-label={t("访问令牌")}><Flex direction="column" gap="3">
  <Heading size="3">{t("访问令牌")}</Heading><Text size="2" color="gray">{t("为 CLI 和 CI 创建独立令牌，使用当前账户的资源权限。原文仅在创建时显示。")}</Text>
  {!selected&&(error||tokens.error)&&<Callout.Root color="red" role="alert"><Callout.Text>{error||liveError(tokens.error)}</Callout.Text></Callout.Root>}
  <form onSubmit={create}><Flex gap="3" wrap="wrap" align="end"><Field label={t("令牌名称")}><TextField.Root required maxLength={80} value={name} disabled={pending} onChange={event=>setName(event.target.value)} autoComplete="off"/></Field><Choice disabled={pending} label={t("有效期")} value={days} options={[{value:"7",label:t("7 天")},{value:"30",label:t("30 天")},{value:"90",label:t("90 天")},{value:"365",label:t("365 天")}]} onChange={setDays}/><Button type="submit" loading={pending} disabled={!name.trim()||pending}>{t("创建访问令牌")}</Button></Flex></form>
  {secret?.scope===scope&&<Flex direction="column" gap="2"><Text size="2">{t("请立即复制并安全保存；关闭此视图后无法再次查看。")}</Text><TextArea aria-label={t("新访问令牌")} readOnly value={secret.value} spellCheck={false}/><Flex gap="2"><Button variant="soft" onClick={()=>void copy()}>{copied?t("已复制"):t("复制令牌")}</Button><Button variant="ghost" onClick={()=>{setSecret(null);setCopied(false);}}>{t("隐藏令牌")}</Button></Flex></Flex>}
  {tokens.isPending&&credential&&<Text role="status">{t("正在加载令牌")}</Text>}
  {tokens.data?.length===0&&<Text size="2" color="gray">{t("尚无访问令牌")}</Text>}
  {tokens.data?.map(value=><Flex key={value.id} align="center" justify="between" gap="3" wrap="wrap"><Flex direction="column" gap="1"><Text weight="medium">{value.name}</Text><Text size="1" color="gray">{value.expired?t("已过期"):t("有效")} · {t("到期时间")}: {date(value.expires_at)}</Text></Flex><Button variant="soft" color="red" disabled={pending} onClick={()=>{setError("");setSelected(value);}}>{t("撤销令牌")}</Button></Flex>)}
  <AlertDialog.Root open={!!selected} onOpenChange={open=>{if(!open&&!pending)setSelected(null);}}><AlertDialog.Content><AlertDialog.Title>{t("撤销令牌")}</AlertDialog.Title><AlertDialog.Description>{t("撤销后该令牌立即失效，当前服务实例上此账户正在执行的任务和连接也会停止。")}</AlertDialog.Description><Text as="p" mt="2">{selected?.name}</Text>{error&&<Callout.Root color="red" role="alert"><Callout.Text>{error}</Callout.Text></Callout.Root>}<Flex justify="end" gap="3" mt="4"><AlertDialog.Cancel><Button variant="soft" color="gray" disabled={pending}>{t("取消")}</Button></AlertDialog.Cancel><Button color="red" loading={pending} onClick={()=>void revoke()}>{t("确认撤销")}</Button></Flex></AlertDialog.Content></AlertDialog.Root>
 </Flex></section>;
}
