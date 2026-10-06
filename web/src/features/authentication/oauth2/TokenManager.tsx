import {useEffect,useRef,useState} from "react";
import {Button,Callout,Dialog,Flex,Text,TextField} from "@radix-ui/themes";
import {api,native} from "../../../shared/api";
import {t,useLanguage,translateCopy} from "../../../shared/i18n";
import {errorCopy,LocalizedError} from "../../../shared/i18n/errors";
import {Choice,Field} from "../../../shared/ui";
import {useWorkbench} from "../../workbench/context";
import type {OAuth2Auth,OAuth2Flow,OAuth2Token} from "./types";
export default function TokenManager({config,select}:{config:OAuth2Auth;select:(id:string|null)=>void}) {
 useLanguage();const state=useWorkbench();const [open,setOpen]=useState(false),[cachedTokens,setTokens]=useState<OAuth2Token[]>([]),[cachedBusy,setBusy]=useState(false),[cachedFlow,assignFlow]=useState<OAuth2Flow|null>(null),[code,setCode]=useState(""),[label,setLabel]=useState(""),[cachedError,setError]=useState<ReturnType<typeof errorCopy>|null>(null);
 const [cachedSecret,setSecret]=useState<string|null>(null);
 const identity=JSON.stringify([state.accountId,state.draft?.id,state.draft?.data.active_environment_id,state.request?.id,config,open]);const current=useRef(identity);current.current=identity;
 const [stateOwner,setStateOwner]=useState(identity);const owns=stateOwner===identity;
 const tokens=owns?cachedTokens:[],busy=owns&&cachedBusy,flow=owns?cachedFlow:null,error=owns?cachedError:null,secret=owns?cachedSecret:null;
 const activeFlow=useRef<{owner:string;id:string;stage:string}|null>(null);
 function setFlow(value:OAuth2Flow|null){activeFlow.current=value?{owner:identity,id:value.id,stage:value.stage}:null;assignFlow(value);}
 const running=useRef(false);
 const epoch=useRef(0);useEffect(()=>{epoch.current++;running.current=false;setStateOwner(identity);setTokens([]);setFlow(null);setSecret(null);setError(null);setBusy(false);},[identity]);
 useEffect(()=>()=>{const pending=activeFlow.current;if(pending?.owner===identity&&["pending","running"].includes(pending.stage)){void api(`/api/oauth2/flows/${encodeURIComponent(pending.id)}/cancel`,"POST").catch(()=>{});activeFlow.current=null;}},[identity]);
 const workspace=state.draft?.id;const base=workspace?`/api/workspaces/${encodeURIComponent(workspace)}/oauth2/tokens`:"";
 const input=()=>({workspace_id:workspace,config,label,environment_id:state.draft?.data.active_environment_id,collection_id:state.draft?.data.collections.find(c=>c.requests.some(r=>r.id===state.request?.id))?.id,locals:state.draft?state.localVariables.values(state.draft,state.draft.data.collections.find(c=>c.requests.some(r=>r.id===state.request?.id))?.id,state.draft.data.active_environment_id):[],verify_tls:state.request?.verify_tls??true});
 async function load(owner=identity){if(current.current!==owner)return;const list=await api<OAuth2Token[]>(base);if(current.current===owner)setTokens(list);}
 useEffect(()=>{if(open&&workspace){const owner=identity;void load(owner).catch(e=>{if(current.current===owner)setError(errorCopy(e));});}},[identity]);
 useEffect(()=>{if(!flow||!["pending","running"].includes(flow.stage))return;const owner=identity,id=flow.id;let stopped=false;let timer:ReturnType<typeof setTimeout>;
 const poll=async()=>{try{const next=await api<OAuth2Flow>(`/api/oauth2/flows/${encodeURIComponent(id)}`);if(stopped||current.current!==owner)return;setFlow(next);if(next.stage==="completed"){await load(owner);if(!stopped&&current.current===owner)select(next.token_id);}else if(next.stage==="pending"||next.stage==="running")timer=setTimeout(poll,1500);}catch(e){if(!stopped&&current.current===owner)setError(errorCopy(e));}};
 timer=setTimeout(poll,1500);return()=>{stopped=true;clearTimeout(timer);};},[flow?.id,flow?.stage,identity]);
 async function action(operation:()=>Promise<void>){if(running.current||!workspace)return;running.current=true;const owner=identity,ticket=++epoch.current;setBusy(true);setError(null);try{await operation();}catch(e){if(current.current===owner&&epoch.current===ticket)setError(errorCopy(e));}finally{if(current.current===owner&&epoch.current===ticket){running.current=false;setBusy(false);}}}
 async function acquire(){const owner=identity;if(state.dirty&&!(await state.save(true)))return;if(current.current!==owner)return;
 if(["client_credentials","password"].includes(config.grant)){const token=await api<OAuth2Token>("/api/oauth2/tokens/acquire","POST",input());if(current.current!==owner)return;await load(owner);if(current.current===owner)select(token.id);}
 else{const next=await api<OAuth2Flow>("/api/oauth2/flows","POST",input());if(current.current===owner){setCode("");setFlow(next);}}
 }
 async function launch(url:string){if(native){const {openUrl}=await import("@tauri-apps/plugin-opener");await openUrl(url);}else window.open(url,"_blank","noopener,noreferrer");}
 async function complete(){if(!flow)return;const owner=identity;const redirect=new URL(code);const values=new URLSearchParams(config.grant==="implicit"?redirect.hash.slice(1):redirect.search);
 const stateValue=values.get("state");if(!stateValue)throw new LocalizedError("回调 URL 缺少 state");
 const implicit=config.grant==="implicit"?{access_token:values.get("access_token"),token_type:values.get("token_type"),expires_in:values.has("expires_in")?Number(values.get("expires_in")):undefined,scope:values.get("scope")??undefined}:undefined;
 const result=await api<OAuth2Flow>(`/api/oauth2/flows/${encodeURIComponent(flow.id)}/complete`,"POST",{state:stateValue,code:values.get("code"),implicit});if(current.current!==owner)return;setFlow(result);setCode("");if(result.stage==="completed"){await load(owner);if(current.current===owner)select(result.token_id);}
 }
 return <><Button variant="soft" color="gray" onClick={()=>setOpen(true)} disabled={!workspace}>{t("管理 OAuth2 Token")}</Button><Text size="1" color="gray">{config.token_id?t("已选择 Token：{{value0}}",{value0:config.token_id}):t("尚未选择 Token")}</Text>
 <Dialog.Root open={open} onOpenChange={setOpen}><Dialog.Content maxWidth="min(800px, calc(100vw - 24px))" onKeyDown={event=>{if(event.ctrlKey||event.metaKey)event.stopPropagation();}}>
 <Dialog.Title>{t("OAuth2 Token 管理")}</Dialog.Title><Dialog.Description>{t("凭据仅用于当前账户和工作区。授权后选择 Token，再发送请求。")}</Dialog.Description>
 <Flex direction="column" gap="3" mt="4"><Field label={t("Token 名称")}><TextField.Root value={label} onChange={e=>setLabel(e.target.value)} maxLength={128}/></Field>
 <Choice label={t("选择 Token")} value={config.token_id??"none"} options={[{value:"none",label:t("尚未选择 Token")},...tokens.map(token=>({value:token.id,label:`${token.label} · ${token.token_type}`}))]} onChange={id=>{setSecret(null);select(id==="none"?null:id);}}/>
 <Flex gap="2" wrap="wrap"><Button disabled={busy} onClick={()=>void action(acquire)}>{t("获取 Token")}</Button><Button variant="soft" disabled={busy||!config.token_id} onClick={()=>void action(async()=>{const owner=identity;await api(`${base}/${encodeURIComponent(config.token_id!)}/refresh`,"POST",input());await load(owner);})}>{t("刷新 Token")}</Button>
 <Button variant="soft" color="gray" disabled={busy||!config.token_id} onClick={()=>void action(async()=>{const owner=identity;const value=await api<{access_token:string}>(`${base}/${encodeURIComponent(config.token_id!)}/secret`);if(current.current===owner)setSecret(value.access_token);})}>{t("显示 Token")}</Button>
 <Button variant="soft" color="red" disabled={busy||!config.token_id} onClick={()=>void action(async()=>{const owner=identity;await api(`${base}/${encodeURIComponent(config.token_id!)}`,"DELETE");if(current.current===owner){select(null);setSecret(null);}await load(owner);})}>{t("删除 Token")}</Button></Flex>
 {secret&&<Field label="Access Token"><TextField.Root readOnly value={secret}/></Field>}
 {flow&&<><Text size="2">{t("授权状态：{{value0}}",{value0:flow.stage})}</Text>{flow.user_code&&<Text className="mono">{flow.user_code}</Text>}{(flow.authorization_url||flow.verification_uri)&&<Button variant="soft" disabled={busy} onClick={()=>void action(()=>launch((flow.authorization_url??flow.verification_uri)!))}>{t("在浏览器中授权")}</Button>}
 {config.grant!=="device_code"&&flow.stage==="pending"&&<><Field label={t("粘贴授权后的回调 URL")}><TextField.Root type="password" autoComplete="off" value={code} onChange={e=>setCode(e.target.value)}/></Field><Button disabled={busy||!code} onClick={()=>void action(complete)}>{t("完成授权")}</Button></>}
 {flow.stage==="pending"&&<Button variant="soft" color="gray" onClick={()=>void action(async()=>{const owner=identity;const value=await api<OAuth2Flow>(`/api/oauth2/flows/${encodeURIComponent(flow.id)}/cancel`,"POST");if(current.current===owner)setFlow(value);})}>{t("取消授权")}</Button>}{flow.error&&<Callout.Root color="red"><Callout.Text>{flow.error}</Callout.Text></Callout.Root>}</>}
 {error&&<Callout.Root color="red" role="alert"><Callout.Text>{translateCopy(error)}</Callout.Text></Callout.Root>}
 <Flex justify="end"><Dialog.Close><Button variant="soft" color="gray">{t("完成")}</Button></Dialog.Close></Flex></Flex>
 </Dialog.Content></Dialog.Root></>;
}
