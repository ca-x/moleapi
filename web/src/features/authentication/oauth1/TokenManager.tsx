import {useContext,useEffect,useRef,useState} from "react";
import {Button,Callout,Dialog,Flex,Text,TextField} from "@radix-ui/themes";
import {api,native} from "../../../shared/api";
import {Choice,Field} from "../../../shared/ui";
import {t,useLanguage,translateCopy} from "../../../shared/i18n";
import {errorCopy,LocalizedError} from "../../../shared/i18n/errors";
import {WorkbenchContext,useWorkbench} from "../../workbench/context";
import type {OAuth1Auth} from "../../../shared/types";
interface Token {id:string;label:string;issuer:string;consumer_key:string;created_at:number;profile_version:number}
interface Flow {id:string;stage:string;authorization_url:string|null;expires_at:number;token_id:string|null;error:string|null}
type Props={config:OAuth1Auth;select:(id:string|null)=>void;callback:(url:string)=>void;collectionId?:string|null};
export default function TokenManager(props:Props){
 const state=useContext(WorkbenchContext);useLanguage();
 return state?<Manager {...props} state={state}/>:<Button variant="soft" disabled>{t("管理 OAuth1 Token")}</Button>;
}
function Manager({config,select,callback,collectionId,state}:Props&{state:ReturnType<typeof useWorkbench>}){
 const [open,setOpen]=useState(false),[tokens,setTokens]=useState<Token[]>([]),[flow,assignFlow]=useState<Flow|null>(null),[busy,setBusy]=useState(false),[label,setLabel]=useState(""),[code,setCode]=useState(""),[secret,setSecret]=useState<{token:string;secret:string}|null>(null),[importToken,setImportToken]=useState(""),[importSecret,setImportSecret]=useState("");
 const [error,setError]=useState<ReturnType<typeof errorCopy>|null>(null),[mode,setMode]=useState<"manual"|"hosted"|"loopback">("manual");
 const workspace=state.draft?.id,collection=collectionId===undefined?state.draft?.data.collections.find(c=>c.requests.some(r=>r.id===state.request?.id))?.id:collectionId;
 const ready=collection==null||!!state.draft?.data.collections.some(c=>c.id===collection);
 const parentScopes:unknown[]=[];const seen=new Set<string>();let ancestor=collection;
 while(ancestor&&parentScopes.length<16&&!seen.has(ancestor)){seen.add(ancestor);const c=state.draft?.data.collections.find(c=>c.id===ancestor);if(!c)break;parentScopes.push([c.id,c.parent_id,c.variables,c.variables_enabled]);ancestor=c.parent_id;}
 const variables=state.draft?[state.draft.data.global_variables,state.draft.data.environments?.find(e=>e.id===state.draft?.data.active_environment_id)?.variables,parentScopes,state.localVariables.values(state.draft,collection??undefined,state.draft.data.active_environment_id)]:[];
 const identity=JSON.stringify([state.accountId,workspace,state.draft?.data.active_environment_id,state.request?.id,collectionId,{...config,token_id:null},variables,open]);
 const current=useRef(identity),generation=useRef(0);if(current.current!==identity){generation.current++;current.current=identity;}
 const [stateOwner,setStateOwner]=useState(identity);const owns=stateOwner===identity;
 const active=useRef<{owner:string;flow:Flow}|null>(null),running=useRef(false),epoch=useRef(0);
 const base=`/api/workspaces/${encodeURIComponent(workspace??"")}/oauth1/tokens`;
 const pending=owns&&flow&&["requesting","pending","running"].includes(flow.stage);
 const uri=native?"http://127.0.0.1:49153/api/oauth1/callback":`${window.location.origin}/api/oauth1/callback`;
 const input=()=>({workspace_id:workspace,config,label,environment_id:state.draft?.data.active_environment_id,collection_id:collection,locals:state.draft?state.localVariables.values(state.draft,collection??undefined,state.draft.data.active_environment_id):[],verify_tls:collectionId===undefined?(state.request?.verify_tls??true):true});
 const cancel=async(id:string)=>{await api(`/api/oauth1/flows/${encodeURIComponent(id)}/cancel`,"POST");};
 function setFlow(value:Flow|null){active.current=value?{owner:identity,flow:value}:null;assignFlow(value);}
 async function load(owner=generation.current){const values=await api<Token[]>(base);if(owner===generation.current)setTokens(values);}
 useEffect(()=>{
   const owner=generation.current;setStateOwner(identity);setTokens([]);assignFlow(null);setSecret(null);setCode("");setImportToken("");setImportSecret("");setError(null);setBusy(false);running.current=false;epoch.current++;
   if(open&&workspace)void load(owner).catch(e=>{if(owner===generation.current)setError(errorCopy(e));});
   return()=>{generation.current++;const previous=active.current;if(previous?.owner===identity){if(["requesting","pending","running"].includes(previous.flow.stage))void cancel(previous.flow.id).catch(()=>{});active.current=null;}};
 },[identity]);
 useEffect(()=>{
   if(!owns||!flow||!["requesting","pending","running"].includes(flow.stage))return;
   const owner=generation.current,id=flow.id;let stopped=false;let timer:ReturnType<typeof setTimeout>;
   const poll=async()=>{try{const next=await api<Flow>(`/api/oauth1/flows/${encodeURIComponent(id)}`);if(stopped||owner!==generation.current)return;
     if(next.stage==="completed"){await load(owner);if(!stopped&&owner===generation.current){setFlow(next);select(next.token_id);}}
     else{setFlow(next);if(["requesting","pending","running"].includes(next.stage))timer=setTimeout(poll,1000);}
   }catch(e){if(!stopped&&owner===generation.current)setError(errorCopy(e));}};
   timer=setTimeout(poll,1000);return()=>{stopped=true;clearTimeout(timer);};
 },[flow?.id,flow?.stage,identity,owns]);
 async function action(operation:(owner:number)=>Promise<void>){if(running.current||!workspace)return;const owner=generation.current,ticket=++epoch.current;running.current=true;setBusy(true);setError(null);try{await operation(owner);}catch(e){if(owner===generation.current&&ticket===epoch.current)setError(errorCopy(e));}finally{if(owner===generation.current&&ticket===epoch.current){running.current=false;setBusy(false);}}}
 async function ensureSaved(owner:number){if(state.dirty&&!(await state.save(true)))return false;return owner===generation.current;}
 async function begin(owner:number){if(!(await ensureSaved(owner)))return;const next=await api<Flow>("/api/oauth1/flows","POST",{...input(),callback_mode:mode});if(owner===generation.current){setCode("");setFlow(next);}else{void cancel(next.id).catch(()=>{});}}
 async function launch(owner:number){if(!flow?.authorization_url)return;if(native){const{openUrl}=await import("@tauri-apps/plugin-opener");if(owner===generation.current)await openUrl(flow.authorization_url);}else window.open(flow.authorization_url,"_blank","noopener,noreferrer");}
 async function complete(owner:number){if(!flow)return;let verifier=code.trim(),token:string|undefined,nonce:string|undefined;
   if(/^https?:\/\//i.test(verifier)){let url:URL;try{url=new URL(verifier);}catch{throw new LocalizedError("回调 URL 无效");}verifier=url.searchParams.get("oauth_verifier")??"";token=url.searchParams.get("oauth_token")??undefined;nonce=url.searchParams.get("state")??undefined;}
   if(!verifier)throw new LocalizedError("请输入 OAuth1 Verifier 或回调 URL");
   const next=await api<Flow>(`/api/oauth1/flows/${encodeURIComponent(flow.id)}/complete`,"POST",{verifier,token,state:nonce});if(owner!==generation.current)return;setCode("");setFlow(next);if(next.stage==="completed"){await load(owner);if(owner===generation.current)select(next.token_id);}}
 return <><Button variant="soft" color="gray" disabled={!workspace||!ready} onClick={()=>setOpen(true)}>{t("管理 OAuth1 Token")}</Button><Text size="1" color="gray">{config.token_id?t("已选择 Token：{{value0}}",{value0:config.token_id}):t("未选择私有 Token，使用手动凭据。")}</Text>
 <Dialog.Root open={open} onOpenChange={setOpen}><Dialog.Content maxWidth="min(800px, calc(100vw - 24px))" onKeyDown={e=>{if(e.ctrlKey||e.metaKey)e.stopPropagation();}}><Dialog.Title>{t("OAuth1 Token 管理")}</Dialog.Title><Dialog.Description>{t("Token 与 Secret 仅保存在当前账户的工作区私有令牌库，不参与同步或导出。")}</Dialog.Description>
 <Flex direction="column" gap="3" mt="4">
 <Field label={t("Token 名称")}><TextField.Root disabled={busy||!!pending} value={label} maxLength={128} onChange={e=>setLabel(e.target.value)}/></Field>
 <Choice disabled={busy||!!pending} label={t("选择 Token")} value={config.token_id??"none"} options={[{value:"none",label:t("使用手动凭据")},...(owns?tokens:[]).map(token=>({value:token.id,label:`${token.label} · ${token.consumer_key}`}))]} onChange={id=>{setSecret(null);select(id==="none"?null:id);}}/>
 <Choice disabled={busy||!!pending} label={t("回调方式")} value={mode} options={[{value:"manual",label:t("手动输入 PIN 或回调 URL")},{value:native?"loopback":"hosted",label:native?t("桌面本地自动回调"):t("服务端自动回调")}]} onChange={value=>setMode(value)}/>
 {mode!=="manual"&&<><Text size="2">{t("注册此基础回调 URI。MoleAPI 会自动附加随机 state，并校验回调中的请求 Token。")}</Text><Text size="2" className="mono">{uri}</Text><Button variant="soft" disabled={busy||!!pending} onClick={()=>callback(uri)}>{t("使用此回调 URI")}</Button></>}
 <Flex gap="2" wrap="wrap"><Button disabled={busy||!!pending} onClick={()=>void action(begin)}>{t("获取 Token")}</Button><Button variant="soft" disabled={busy||!config.token_id} onClick={()=>void action(async owner=>{const value=await api<{token:string;secret:string}>(`${base}/${encodeURIComponent(config.token_id!)}/secret`);if(owner===generation.current)setSecret(value);})}>{t("显示 Token")}</Button><Button variant="soft" disabled={busy||!config.token_id||!label.trim()} onClick={()=>void action(async owner=>{await api(`${base}/${encodeURIComponent(config.token_id!)}`,"PATCH",{label});await load(owner);})}>{t("重命名 Token")}</Button><Button variant="soft" color="red" disabled={busy||!config.token_id} onClick={()=>void action(async owner=>{await api(`${base}/${encodeURIComponent(config.token_id!)}`,"DELETE");if(owner===generation.current){select(null);setSecret(null);}await load(owner);})}>{t("删除本地 Token")}</Button></Flex>
 <Text size="1" color="gray">{t("删除只移除本地凭据，不会撤销授权服务中的 Token。OAuth1 没有统一的刷新或撤销端点。")}</Text>
 {owns&&flow&&<><Text size="2" role="status">{t("授权状态：{{value0}}",{value0:({requesting:t("正在获取请求 Token"),pending:t("等待授权"),running:t("正在完成授权"),completed:t("授权完成"),cancelled:t("授权已取消"),failed:t("授权失败")} as Record<string,string>)[flow.stage]??flow.stage})}</Text>
 {flow.authorization_url&&flow.stage==="pending"&&<Button variant="soft" disabled={busy} onClick={()=>void action(launch)}>{t("在浏览器中授权")}</Button>}
 {flow.stage==="pending"&&<><Field label={t("OAuth1 Verifier / PIN 或回调 URL")}><TextField.Root type="password" autoComplete="off" maxLength={16384} value={code} onChange={e=>setCode(e.target.value)}/></Field><Button disabled={busy||!code} onClick={()=>void action(complete)}>{t("完成授权")}</Button></>}
 {pending&&<Button variant="soft" color="gray" onClick={()=>{const owner=generation.current;void api<Flow>(`/api/oauth1/flows/${encodeURIComponent(flow.id)}/cancel`,"POST").then(async result=>{if(owner!==generation.current)return;generation.current++;epoch.current++;running.current=false;setBusy(false);setCode("");setFlow(result);const nextOwner=generation.current;await load(nextOwner).catch(e=>{if(nextOwner===generation.current)setError(errorCopy(e));});}).catch(e=>{if(owner===generation.current)setError(errorCopy(e));});}}>{t("取消授权")}</Button>}
 {flow.error&&<Callout.Root color="red" role="alert"><Callout.Text>{flow.error}</Callout.Text></Callout.Root>}</>}
 {owns&&secret&&<><Field label="Token"><TextField.Root readOnly value={secret.token}/></Field><Field label="Token Secret"><TextField.Root readOnly value={secret.secret}/></Field><Button variant="soft" onClick={()=>void action(async()=>{await navigator.clipboard.writeText(JSON.stringify(secret));})}>{t("复制 Token 与 Secret")}</Button></>}
 <details><summary>{t("导入已有 Token")}</summary><Flex direction="column" gap="3" mt="3"><Field label="Token"><TextField.Root disabled={busy} type="password" autoComplete="off" maxLength={4096} value={importToken} onChange={e=>setImportToken(e.target.value)}/></Field><Field label="Token Secret"><TextField.Root disabled={busy} type="password" autoComplete="off" maxLength={65536} value={importSecret} onChange={e=>setImportSecret(e.target.value)}/></Field><Button disabled={busy||!importToken||!importSecret} onClick={()=>void action(async owner=>{if(!(await ensureSaved(owner)))return;const token=await api<Token>("/api/oauth1/tokens/import","POST",{...input(),token:{token:importToken,secret:importSecret}});if(owner!==generation.current)return;setImportToken("");setImportSecret("");await load(owner);if(owner===generation.current)select(token.id);})}>{t("导入 Token")}</Button></Flex></details>
 {owns&&error&&<Callout.Root color="red" role="alert"><Callout.Text>{translateCopy(error)}</Callout.Text></Callout.Root>}
 {owns&&busy&&<Text role="status" size="2">{t("正在加载…")}</Text>}
 <Flex justify="end"><Dialog.Close><Button variant="soft" color="gray">{t("完成")}</Button></Dialog.Close></Flex>
 </Flex></Dialog.Content></Dialog.Root></>;
}
